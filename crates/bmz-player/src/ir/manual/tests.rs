use std::path::PathBuf;
use std::time::{Duration, Instant};

use rusqlite::Connection;

use super::*;
use crate::config::profile_config::IrProviderConfig;
use crate::ir::backfill::IrLocalUploadOptions;
use crate::ir::sync::IrSyncReport;
use crate::ln_policy::LnScorePolicy;
use crate::storage::common::configure_connection;
use crate::storage::migration::{
    NETWORK_MIGRATIONS, migrate_library_db, migrate_network_db, migrate_score_db, run_migrations,
};
use crate::storage::network_db::{IrJobKind, NetworkDatabase, NewIrScoreJob};

use super::sync::{IrManualSyncScope, sync_jobs};
use super::upload::full_upload_stop_reason;

fn temp_dir(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "bmz-ir-manual-{name}-{}-{}",
        std::process::id(),
        stamp.as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_network_db() -> NetworkDatabase {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, NETWORK_MIGRATIONS).unwrap();
    NetworkDatabase::from_connection(conn)
}

/// 設定に存在しないproviderのjob。送信は通信せずローカルで失敗扱いになる。
fn enqueue_unroutable_job(db: &mut NetworkDatabase, local_score_id: i64) -> i64 {
    db.enqueue_ir_score_job(&NewIrScoreJob {
        provider: "unconfigured".to_string(),
        account_id: "account-1".to_string(),
        kind: IrJobKind::Score,
        local_score_id,
        chart_sha256: [local_score_id as u8; 32],
        ln_policy: LnScorePolicy::ForceLn,
        payload_json: "{}".to_string(),
        now: 1,
    })
    .unwrap()
}

struct Fixture {
    dir: PathBuf,
    score_db: PathBuf,
    config: IrConfig,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = temp_dir(name);
        let score_db = dir.join("score.db");
        crate::storage::migration::migrate_score_db(&score_db).unwrap();
        Self { dir, score_db, config: IrConfig::default() }
    }

    fn ctx(&self) -> IrSyncContext<'_> {
        IrSyncContext {
            score_db: &self.score_db,
            profile_root: &self.dir,
            logs_dir: &self.dir,
            ir_config: &self.config,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn full_upload_distinguishes_failures_from_waiting_for_a_sending_job() {
    assert_eq!(
        full_upload_stop_reason(&IrSyncReport::default()),
        Some(IrManualStop::WaitingForSendingJob)
    );
    assert_eq!(full_upload_stop_reason(&IrSyncReport { submitted: 1, ..Default::default() }), None);
    assert_eq!(
        full_upload_stop_reason(&IrSyncReport { submitted: 1, failed: 2, ..Default::default() }),
        Some(IrManualStop::FailedJobs { failed: 2 })
    );
}

#[tokio::test]
async fn sync_jobs_reports_progress_for_every_job() {
    let fixture = Fixture::new("progress");
    let mut db = open_network_db();
    for id in 1..=2 {
        enqueue_unroutable_job(&mut db, id);
    }
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let mut events = Vec::new();
    let outcome = sync_jobs(
        &fixture.ctx(),
        &mut db,
        IrManualSyncScope::Any,
        10,
        &mut |event| events.push(event),
        &cancel,
    )
    .await
    .unwrap();

    assert!(!outcome.cancelled);
    assert_eq!(outcome.report.failed, 2);
    assert_eq!(outcome.report.submitted, 0);
    assert!(matches!(events[0], IrManualProgress::SyncStarted { limit: 10, .. }));
    let started: Vec<u32> = events
        .iter()
        .filter_map(|event| match event {
            IrManualProgress::JobSyncing { index, .. } => Some(*index),
            _ => None,
        })
        .collect();
    assert_eq!(started, [1, 2]);
    let finished = events
        .iter()
        .filter(|event| matches!(event, IrManualProgress::JobFinished { failed: 1, .. }))
        .count();
    assert_eq!(finished, 2);
}

#[tokio::test]
async fn sync_jobs_stops_before_the_next_job_when_cancelled() {
    let fixture = Fixture::new("cancel");
    let mut db = open_network_db();
    for id in 1..=3 {
        enqueue_unroutable_job(&mut db, id);
    }
    let (tx, cancel) = tokio::sync::watch::channel(false);
    let outcome = sync_jobs(
        &fixture.ctx(),
        &mut db,
        IrManualSyncScope::Any,
        10,
        &mut |event| {
            if matches!(event, IrManualProgress::JobFinished { .. }) {
                tx.send_replace(true);
            }
        },
        &cancel,
    )
    .await
    .unwrap();

    assert!(outcome.cancelled);
    // 処理中の1件は完了し、残りは手を付けない。
    assert_eq!(outcome.report.failed, 1);
    let untouched = db.pending_ir_score_jobs(i64::MAX / 2, 10).unwrap();
    assert_eq!(untouched.iter().filter(|job| job.status == "pending").count(), 2);
}

#[tokio::test]
async fn sync_jobs_does_not_start_when_already_cancelled() {
    let fixture = Fixture::new("precancel");
    let mut db = open_network_db();
    enqueue_unroutable_job(&mut db, 1);
    let (_tx, cancel) = tokio::sync::watch::channel(true);
    let outcome =
        sync_jobs(&fixture.ctx(), &mut db, IrManualSyncScope::Any, 10, &mut |_| {}, &cancel)
            .await
            .unwrap();
    assert!(outcome.cancelled);
    assert_eq!(outcome.report.failed, 0);
}

#[tokio::test]
async fn sync_jobs_skips_jobs_another_worker_is_sending() {
    let fixture = Fixture::new("sending");
    let mut db = open_network_db();
    enqueue_unroutable_job(&mut db, 1);
    // 常駐workerなど別の主体がclaim済み（lease内）。
    let claimed = db.claim_pending_ir_score_jobs(now_unix_seconds(), 10, true).unwrap();
    assert_eq!(claimed.len(), 1);

    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let mut events = Vec::new();
    let outcome = sync_jobs(
        &fixture.ctx(),
        &mut db,
        IrManualSyncScope::Any,
        10,
        &mut |event| events.push(event),
        &cancel,
    )
    .await
    .unwrap();

    assert_eq!(outcome.report.submitted, 0);
    assert_eq!(outcome.report.failed, 0);
    assert!(events.iter().any(|event| matches!(event, IrManualProgress::NoQueuedJobs)));
    // 進捗なし（失敗でもない）= lease待ちとして扱われる。
    assert_eq!(full_upload_stop_reason(&outcome.report), Some(IrManualStop::WaitingForSendingJob));
}

struct UploadFixture {
    dir: PathBuf,
    library_db: PathBuf,
    score_db: PathBuf,
    network_db: PathBuf,
    config: IrConfig,
}

impl UploadFixture {
    fn new(name: &str) -> Self {
        let dir = temp_dir(name);
        let library_db = dir.join("library.db");
        let score_db = dir.join("score.db");
        let network_db = dir.join("network.db");
        migrate_library_db(&library_db).unwrap();
        migrate_score_db(&score_db).unwrap();
        migrate_network_db(&network_db).unwrap();

        let mut target = IrProviderConfig::bmz_ir();
        target.provider_key = "target-a".to_string();
        target.enabled = true;
        target.account_id = "account-a".to_string();
        let config = IrConfig {
            primary_provider: "target-a".to_string(),
            providers: vec![target],
            ..IrConfig::default()
        };
        Self { dir, library_db, score_db, network_db, config }
    }

    fn context(&self) -> IrManualContext<'_> {
        IrManualContext {
            library_db: &self.library_db,
            network_db: &self.network_db,
            sync: IrSyncContext {
                score_db: &self.score_db,
                profile_root: &self.dir,
                logs_dir: &self.dir,
                ir_config: &self.config,
            },
        }
    }

    fn seed_target_sending_and_unrelated_pending(&self) -> Vec<(i64, &'static str)> {
        let mut db = NetworkDatabase::open(&self.network_db).unwrap();
        let now = now_unix_seconds();
        let target_score =
            enqueue_upload_job(&mut db, "target-a", "account-a", IrJobKind::Score, 1, now);
        let claimed = db
            .claim_pending_ir_score_jobs_for_kind(
                "target-a",
                "account-a",
                IrJobKind::Score,
                now,
                1,
                true,
            )
            .unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, target_score);

        let other_provider =
            enqueue_upload_job(&mut db, "unconfigured", "other", IrJobKind::Score, 2, now);
        let other_account =
            enqueue_upload_job(&mut db, "target-a", "account-b", IrJobKind::Score, 3, now);
        let replay =
            enqueue_upload_job(&mut db, "target-a", "account-a", IrJobKind::Replay, 4, now);
        let course =
            enqueue_upload_job(&mut db, "target-a", "account-a", IrJobKind::Course, 5, now);
        vec![
            (target_score, "sending"),
            (other_provider, "pending"),
            (other_account, "pending"),
            (replay, "pending"),
            (course, "pending"),
        ]
    }

    fn seed_only_unrelated_pending(&self) -> Vec<(i64, &'static str)> {
        let mut db = NetworkDatabase::open(&self.network_db).unwrap();
        let now = now_unix_seconds();
        let other_provider =
            enqueue_upload_job(&mut db, "unconfigured", "other", IrJobKind::Score, 12, now);
        let other_account =
            enqueue_upload_job(&mut db, "target-a", "account-b", IrJobKind::Score, 13, now);
        let replay =
            enqueue_upload_job(&mut db, "target-a", "account-a", IrJobKind::Replay, 14, now);
        let course =
            enqueue_upload_job(&mut db, "target-a", "account-a", IrJobKind::Course, 15, now);
        vec![
            (other_provider, "pending"),
            (other_account, "pending"),
            (replay, "pending"),
            (course, "pending"),
        ]
    }

    fn assert_job_statuses(&self, expected: &[(i64, &str)]) {
        let db = NetworkDatabase::open(&self.network_db).unwrap();
        for (job_id, expected_status) in expected {
            let status: String = db
                .conn()
                .query_row("SELECT status FROM ir_score_jobs WHERE id = ?1", [job_id], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(&status, expected_status, "job {job_id}");
        }
    }
}

impl Drop for UploadFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn enqueue_upload_job(
    db: &mut NetworkDatabase,
    provider: &str,
    account_id: &str,
    kind: IrJobKind,
    local_score_id: i64,
    now: i64,
) -> i64 {
    db.enqueue_ir_score_job(&NewIrScoreJob {
        provider: provider.to_string(),
        account_id: account_id.to_string(),
        kind,
        local_score_id,
        chart_sha256: [local_score_id as u8; 32],
        ln_policy: LnScorePolicy::ForceLn,
        payload_json: "{}".to_string(),
        now,
    })
    .unwrap()
}

#[tokio::test]
async fn upload_local_all_waits_for_its_sending_score_without_touching_other_jobs() {
    let fixture = UploadFixture::new("upload-all-scope");
    let expected = fixture.seed_target_sending_and_unrelated_pending();
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let report = upload_local_all(
        &fixture.context(),
        &IrLocalUploadOptions::default(),
        &mut |_| {},
        &cancel,
    )
    .await
    .unwrap();

    assert_eq!(report.stop, IrManualStop::WaitingForSendingJob);
    assert_eq!(report.remaining, 1);
    fixture.assert_job_statuses(&expected);
}

#[tokio::test]
async fn upload_local_batch_sync_waits_for_its_sending_score_without_touching_other_jobs() {
    let fixture = UploadFixture::new("upload-batch-scope");
    let expected = fixture.seed_target_sending_and_unrelated_pending();
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let report = upload_local_batch(
        &fixture.context(),
        &IrLocalUploadOptions::default(),
        true,
        &mut |_| {},
        &cancel,
    )
    .await
    .unwrap();

    assert_eq!(report.stop, IrManualStop::WaitingForSendingJob);
    assert_eq!(report.remaining, 1);
    fixture.assert_job_statuses(&expected);
}

#[tokio::test]
async fn empty_upload_local_batch_completes_without_touching_other_jobs() {
    let fixture = UploadFixture::new("upload-batch-empty");
    let expected = fixture.seed_only_unrelated_pending();
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let report = upload_local_batch(
        &fixture.context(),
        &IrLocalUploadOptions::default(),
        true,
        &mut |_| {},
        &cancel,
    )
    .await
    .unwrap();

    assert_eq!(report.stop, IrManualStop::Completed);
    assert_eq!(report.remaining, 0);
    fixture.assert_job_statuses(&expected);
}

#[tokio::test]
async fn sleep_or_cancel_returns_early_on_cancel() {
    let (tx, cancel) = tokio::sync::watch::channel(false);
    let started = Instant::now();
    let sleeper =
        tokio::spawn(async move { sleep_or_cancel(Duration::from_secs(30), &cancel).await });
    tokio::time::sleep(Duration::from_millis(30)).await;
    tx.send_replace(true);
    assert!(sleeper.await.unwrap());
    assert!(started.elapsed() < Duration::from_secs(5));

    let (_tx, cancel) = tokio::sync::watch::channel(false);
    assert!(!sleep_or_cancel(Duration::from_millis(5), &cancel).await);
}

#[tokio::test]
async fn resend_job_ignores_backoff_and_skips_a_job_being_sent() {
    let fixture = Fixture::new("resend-one");
    let mut db = open_network_db();
    let failed = enqueue_unroutable_job(&mut db, 1);
    let busy = enqueue_unroutable_job(&mut db, 2);
    db.mark_ir_score_job_failed(failed, now_unix_seconds(), "boom", None).unwrap();
    // 失敗直後なので通常のclaimでは待機中。
    assert!(db.claim_pending_ir_score_jobs(now_unix_seconds(), 1, false).unwrap()[0].id == busy);

    let report =
        resend_job(&fixture.ctx(), &mut db, "unconfigured", "account-1", IrJobKind::Score, 1)
            .await
            .unwrap();
    // 再送が実際に試行された（未設定providerなので再度失敗扱い）。
    assert_eq!((report.targeted, report.failed, report.skipped_busy), (1, 1, 0));
    let counts = db.ir_score_job_status_counts(None, None).unwrap();
    assert_eq!(counts.failed, 1);

    // sending中のjobは二重に送らない。
    let report =
        resend_job(&fixture.ctx(), &mut db, "unconfigured", "account-1", IrJobKind::Score, 2)
            .await
            .unwrap();
    assert_eq!((report.failed, report.skipped_busy), (0, 1));
}

#[tokio::test]
async fn resend_failed_jobs_only_targets_failed_jobs_and_can_be_cancelled() {
    let fixture = Fixture::new("resend-bulk");
    let mut db = open_network_db();
    let failed_ids: Vec<i64> = (1..=3).map(|id| enqueue_unroutable_job(&mut db, id)).collect();
    let pending = enqueue_unroutable_job(&mut db, 9);
    for id in &failed_ids {
        db.mark_ir_score_job_failed(*id, now_unix_seconds(), "boom", None).unwrap();
    }

    let (tx, cancel) = tokio::sync::watch::channel(false);
    let mut finished = 0;
    let report = resend_failed_jobs(
        &fixture.ctx(),
        &mut db,
        "unconfigured",
        "account-1",
        &mut |event| {
            if matches!(event, IrManualProgress::JobFinished { .. }) {
                finished += 1;
                if finished == 2 {
                    tx.send_replace(true);
                }
            }
        },
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(report.targeted, 3);
    assert!(report.cancelled);
    assert_eq!(report.failed, 2);

    // pendingのjobは触らない。
    let jobs = db.pending_ir_score_jobs(i64::MAX / 2, 10).unwrap();
    assert!(jobs.iter().any(|job| job.id == pending && job.status == "pending"));

    // 中断なしなら残りも含めて全件が対象になる。
    let (_tx, cancel) = tokio::sync::watch::channel(false);
    let report = resend_failed_jobs(
        &fixture.ctx(),
        &mut db,
        "unconfigured",
        "account-1",
        &mut |_| {},
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!((report.targeted, report.failed, report.cancelled), (3, 3, false));
}
