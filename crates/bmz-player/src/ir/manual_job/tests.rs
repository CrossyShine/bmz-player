use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;
use crate::ln_policy::LnScorePolicy;
use crate::storage::network_db::NewIrScoreJob;

struct Fixture {
    dir: PathBuf,
    env: IrManualEnv,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "bmz-ir-manual-job-{name}-{}-{}",
            std::process::id(),
            stamp.as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let env = IrManualEnv {
            library_db: dir.join("library.db"),
            score_db: dir.join("score.db"),
            network_db: dir.join("network.db"),
            profile_root: dir.clone(),
            logs_dir: dir.clone(),
            ir_config: IrConfig::default(),
        };
        crate::storage::migration::migrate_library_db(&env.library_db).unwrap();
        crate::storage::migration::migrate_score_db(&env.score_db).unwrap();
        crate::storage::migration::migrate_network_db(&env.network_db).unwrap();
        Self { dir, env }
    }

    /// 未設定providerのfailed job。再送しても通信せずローカルで失敗する。
    fn add_failed_jobs(&self, count: i64) {
        let mut db = NetworkDatabase::open(&self.env.network_db).unwrap();
        for id in 1..=count {
            let job = db
                .enqueue_ir_score_job(&NewIrScoreJob {
                    provider: "unconfigured".into(),
                    account_id: "account".into(),
                    kind: IrJobKind::Score,
                    local_score_id: id,
                    chart_sha256: [id as u8; 32],
                    ln_policy: LnScorePolicy::ForceLn,
                    payload_json: "{}".into(),
                    now: 1,
                })
                .unwrap();
            db.mark_ir_score_job_failed(job, now_unix_seconds(), "boom", None).unwrap();
        }
    }

    fn resend_all() -> IrManualOp {
        IrManualOp::ResendFailed {
            provider_key: "unconfigured".into(),
            account_id: "account".into(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn run_all() -> IrManualQueued {
    IrManualQueued { generation: 0, request: IrManualRequest::Run(Fixture::resend_all()) }
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn run_pauses_the_resident_worker_and_publishes_the_outcome() {
    let fixture = Fixture::new("run");
    fixture.add_failed_jobs(3);
    let handle = IrManualHandle::new();
    let pause = handle.pause_receiver();
    assert!(!*pause.borrow());

    handle.dispatch(fixture.env.clone(), run_all());
    assert!(handle.is_busy());
    assert!(*pause.borrow(), "worker must be paused while the manual operation runs");
    // 実行中の2つ目の書き込みは受け付けない。
    handle.request(IrManualRequest::Run(IrManualOp::DownloadScores));
    assert!(handle.drain_requests().is_empty());

    wait_until("the operation to finish", || !handle.is_busy());
    assert!(!*pause.borrow(), "worker must resume after the operation");
    let outcome = handle.snapshot().last_outcome.unwrap();
    assert_eq!(outcome.status, IrManualOutcomeStatus::Completed);
    assert_eq!(outcome.progress.failed, 3);
    assert!(handle.take_notice().is_some());
    assert!(handle.take_notice().is_none());
}

#[test]
fn cancel_stops_after_the_job_in_flight() {
    let fixture = Fixture::new("cancel");
    fixture.add_failed_jobs(20);
    let handle = IrManualHandle::new();
    handle.dispatch(fixture.env.clone(), run_all());
    wait_until("the first job", || {
        handle.snapshot().running.is_some_and(|running| running.progress.failed >= 1)
    });
    handle.cancel();
    assert!(handle.snapshot().running.is_none_or(|running| running.progress.cancel_requested));
    wait_until("the operation to stop", || !handle.is_busy());

    let outcome = handle.snapshot().last_outcome.unwrap();
    assert_eq!(outcome.status, IrManualOutcomeStatus::Cancelled);
    assert!(outcome.progress.failed >= 1 && outcome.progress.failed < 20);
    assert!(!*handle.pause_receiver().borrow());
}

#[test]
fn results_of_a_superseded_generation_are_discarded() {
    let fixture = Fixture::new("generation");
    fixture.add_failed_jobs(20);
    let handle = IrManualHandle::new();
    handle.dispatch(fixture.env.clone(), run_all());
    wait_until("the first job", || {
        handle.snapshot().running.is_some_and(|running| running.progress.failed >= 1)
    });
    // profile切替相当。旧世代の進捗・結果・通知は反映されない。
    handle.reset_for_profile();
    let snapshot = handle.snapshot();
    assert!(snapshot.running.is_none() && snapshot.last_outcome.is_none());

    // 旧スレッドが終わるまで待ってから、何も反映されていないことを確認する。
    handle.shutdown(Duration::from_secs(10));
    let snapshot = handle.snapshot();
    assert!(snapshot.running.is_none());
    assert!(snapshot.last_outcome.is_none());
    assert!(handle.take_notice().is_none());
    assert!(!*handle.pause_receiver().borrow());
}

#[test]
fn queue_loads_are_ordered_and_stale_loads_are_dropped() {
    let fixture = Fixture::new("queue");
    fixture.add_failed_jobs(2);
    let handle = IrManualHandle::new();
    let query = IrQueueQuery {
        provider_key: "unconfigured".into(),
        account_id: "account".into(),
        status: IrQueueStatusFilter::Failed,
        page: 0,
    };
    // 1回目の要求は、2回目の要求で世代が進むため反映されない。
    handle.request(IrManualRequest::LoadQueue(query.clone()));
    handle.request(IrManualRequest::LoadQueue(IrQueueQuery { page: 1, ..query.clone() }));
    for request in handle.drain_requests() {
        handle.dispatch(fixture.env.clone(), request);
    }
    wait_until("the queue view", || matches!(handle.snapshot().queue, IrQueueState::Ready(_)));
    std::thread::sleep(Duration::from_millis(100));
    let IrQueueState::Ready(view) = handle.snapshot().queue else { unreachable!() };
    assert_eq!(view.query.page, 1, "only the latest request may be applied");
    assert_eq!(view.counts.failed, 2);
    assert_eq!(view.total, 2);
    assert!(view.rows.is_empty(), "page 1 is past the end");

    handle.request(IrManualRequest::LoadQueue(query));
    for request in handle.drain_requests() {
        handle.dispatch(fixture.env.clone(), request);
    }
    wait_until(
        "page 0",
        || matches!(&handle.snapshot().queue, IrQueueState::Ready(view) if view.query.page == 0),
    );
    let IrQueueState::Ready(view) = handle.snapshot().queue else { unreachable!() };
    assert_eq!(view.rows.len(), 2);
    assert!(view.rows[0].title.is_none());
}

#[test]
fn finishing_an_operation_refreshes_the_open_queue_view() {
    let fixture = Fixture::new("refresh");
    fixture.add_failed_jobs(1);
    let handle = IrManualHandle::new();
    handle.request(IrManualRequest::LoadQueue(IrQueueQuery {
        provider_key: "unconfigured".into(),
        account_id: "account".into(),
        status: IrQueueStatusFilter::Failed,
        page: 0,
    }));
    let _ = handle.drain_requests();
    handle.dispatch(fixture.env.clone(), run_all());
    wait_until("the operation to finish", || !handle.is_busy());
    assert!(
        handle
            .drain_requests()
            .iter()
            .any(|request| matches!(request.request, IrManualRequest::LoadQueue(_)))
    );
}
