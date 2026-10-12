use std::collections::HashSet;

use anyhow::{Result, bail};

use crate::ir::backfill::{
    IrLocalUploadOptions, IrLocalUploadReport, enqueue_local_score_jobs,
    enqueue_local_score_jobs_with_processed, resolve_local_upload_target,
};
use crate::ir::sync::{IR_CLI_SYNC_BATCH_LIMIT, IrSyncReport};
use crate::storage::library_db::LibraryDatabase;
use crate::storage::network_db::{IrJobKind, IrJobRetryFilter, NetworkDatabase};
use crate::storage::score_db::ScoreDatabase;

use super::sync::{
    IrFailureBreaker, IrManualSyncOutcome, IrManualSyncRetry, IrManualSyncScope, sync_jobs_with,
};
use super::{
    IrManualCancel, IrManualContext, IrManualProgress, IrManualProgressSink, IrManualStop,
    IrManualUploadReport, is_cancelled, now_unix_seconds,
};

fn absorb_enqueue(report: &mut IrManualUploadReport, enqueue: &IrLocalUploadReport) {
    report.scanned = report.scanned.saturating_add(enqueue.scanned);
    report.candidates = report.candidates.saturating_add(enqueue.candidates);
    report.enqueued = report.enqueued.saturating_add(enqueue.enqueued);
    report.skipped_already_submitted =
        report.skipped_already_submitted.saturating_add(enqueue.skipped_already_submitted);
    report.skipped_already_queued =
        report.skipped_already_queued.saturating_add(enqueue.skipped_already_queued);
    report.skipped_resend_payload_unavailable = report
        .skipped_resend_payload_unavailable
        .saturating_add(enqueue.skipped_resend_payload_unavailable);
    report.skipped_busy = report.skipped_busy.saturating_add(enqueue.skipped_busy);
    report.skipped_missing_chart =
        report.skipped_missing_chart.saturating_add(enqueue.skipped_missing_chart);
    report.skipped_course_stage =
        report.skipped_course_stage.saturating_add(enqueue.skipped_course_stage);
    report.skipped_autoplay = report.skipped_autoplay.saturating_add(enqueue.skipped_autoplay);
    report.missing_replays = report.missing_replays.saturating_add(enqueue.missing_replays);
    report.limit_reached = enqueue.limit_reached;
}

/// 同期結果から、`--all` を続行できない理由を判定する。続行できるなら `None`。
pub(crate) fn full_upload_stop_reason(sync_report: &IrSyncReport) -> Option<IrManualStop> {
    if sync_report.failed > 0 {
        return Some(IrManualStop::FailedJobs { failed: sync_report.failed });
    }
    if sync_report.submitted == 0 {
        return Some(IrManualStop::WaitingForSendingJob);
    }
    None
}

/// 1回分のsyncを実行し、集計・remaining通知まで行う。
async fn sync_and_summarize(
    ctx: &IrManualContext<'_>,
    network_db: &mut NetworkDatabase,
    report: &mut IrManualUploadReport,
    retry: IrManualSyncRetry,
    breaker: Option<&mut IrFailureBreaker>,
    progress: IrManualProgressSink<'_>,
    cancel: &IrManualCancel,
) -> Result<IrManualSyncOutcome> {
    let outcome = sync_jobs_with(
        &ctx.sync,
        network_db,
        IrManualSyncScope::Kind {
            provider_key: &report.provider_key,
            account_id: &report.account_id,
            kind: IrJobKind::Score,
        },
        IR_CLI_SYNC_BATCH_LIMIT,
        retry,
        breaker,
        progress,
        cancel,
    )
    .await?;
    let remaining = network_db.unfinished_ir_score_job_count_for_kind(
        &report.provider_key,
        &report.account_id,
        IrJobKind::Score,
    )?;
    report.remaining = remaining;
    report.submitted = report.submitted.saturating_add(outcome.report.submitted);
    report.failed = report.failed.saturating_add(outcome.report.failed);
    report.messages.extend(outcome.report.messages.iter().cloned());
    progress(IrManualProgress::SyncSummary {
        provider_key: report.provider_key.clone(),
        account_id: report.account_id.clone(),
        submitted: outcome.report.submitted,
        failed: outcome.report.failed,
        remaining,
        messages: outcome.report.messages.clone(),
    });
    Ok(outcome)
}

fn open_dbs(
    ctx: &IrManualContext<'_>,
) -> Result<(LibraryDatabase, ScoreDatabase, NetworkDatabase)> {
    Ok((
        LibraryDatabase::open(ctx.library_db)?,
        ScoreDatabase::open(ctx.sync.score_db)?,
        NetworkDatabase::open(ctx.network_db)?,
    ))
}

/// `bmz ir upload-local`（`--all` なし）相当。1batchだけenqueueし、必要なら続けて送信する。
pub async fn upload_local_batch(
    ctx: &IrManualContext<'_>,
    options: &IrLocalUploadOptions,
    sync_after_enqueue: bool,
    progress: IrManualProgressSink<'_>,
    cancel: &IrManualCancel,
) -> Result<IrManualUploadReport> {
    let (library_db, score_db, mut network_db) = open_dbs(ctx)?;
    let (provider_key, account_id) =
        resolve_local_upload_target(ctx.sync.ir_config, options.provider.as_deref())?;
    let mut report = IrManualUploadReport {
        provider_key: provider_key.clone(),
        account_id: account_id.clone(),
        batches: 1,
        ..IrManualUploadReport::default()
    };

    if sync_after_enqueue && !options.dry_run {
        let queued = network_db.unfinished_ir_score_job_count_for_kind(
            &provider_key,
            &account_id,
            IrJobKind::Score,
        )?;
        if queued > 0 {
            progress(IrManualProgress::ExistingQueued { provider_key, account_id, queued });
            let outcome = sync_and_summarize(
                ctx,
                &mut network_db,
                &mut report,
                IrManualSyncRetry::UntilFirstFailure,
                None,
                progress,
                cancel,
            )
            .await?;
            report.stop = batch_stop_reason(&outcome, report.remaining);
            return Ok(report);
        }
    }

    let enqueue = enqueue_local_score_jobs(
        ctx.sync.profile_root,
        ctx.sync.ir_config,
        &score_db,
        &library_db,
        &mut network_db,
        options,
        now_unix_seconds(),
    )?;
    absorb_enqueue(&mut report, &enqueue);
    progress(IrManualProgress::Enqueued {
        report: enqueue,
        dry_run: options.dry_run,
        all: false,
        limit: options.limit.max(1),
    });

    if options.dry_run || !sync_after_enqueue {
        return Ok(report);
    }
    let outcome = sync_and_summarize(
        ctx,
        &mut network_db,
        &mut report,
        IrManualSyncRetry::UntilFirstFailure,
        None,
        progress,
        cancel,
    )
    .await?;
    report.stop = batch_stop_reason(&outcome, report.remaining);
    Ok(report)
}

fn batch_stop_reason(outcome: &IrManualSyncOutcome, remaining: u32) -> IrManualStop {
    if outcome.cancelled {
        return IrManualStop::Cancelled;
    }
    if outcome.report.failed > 0 || remaining > 0 {
        return full_upload_stop_reason(&outcome.report).unwrap_or_default();
    }
    IrManualStop::Completed
}

/// `bmz ir upload-local --all` 相当。未完了jobの同期とenqueueを、候補が無くなるまで繰り返す。
///
/// `run_started_at` 以降に失敗したjobは再取得しないため、1回の実行で各jobは最大1回だけ送られる。
/// 失敗したjobは送信キューに残したまま次のjobへ進み、全件を処理し終えたら
/// `IrManualStop::FailedJobs` で終わる。認証エラー・429・連続失敗では
/// `IrFailureBreaker` が途中で打ち切る。他の主体がlease中のjobしか残らない場合は
/// `WaitingForSendingJob` で止まる。再開する呼び出し側は同じ `run_started_at` を渡す。
pub async fn upload_local_all(
    ctx: &IrManualContext<'_>,
    options: &IrLocalUploadOptions,
    run_started_at: i64,
    progress: IrManualProgressSink<'_>,
    cancel: &IrManualCancel,
) -> Result<IrManualUploadReport> {
    if options.dry_run {
        bail!("ir upload-local --all cannot be combined with --dry-run");
    }
    let (provider_key, account_id) =
        resolve_local_upload_target(ctx.sync.ir_config, options.provider.as_deref())?;
    let (library_db, score_db, mut network_db) = open_dbs(ctx)?;
    let mut report = IrManualUploadReport {
        provider_key: provider_key.clone(),
        account_id: account_id.clone(),
        ..IrManualUploadReport::default()
    };
    let retry = IrManualSyncRetry::OncePerRun { started_at: run_started_at };
    let retry_filter = IrJobRetryFilter::FailedBefore(run_started_at);
    let mut breaker = IrFailureBreaker::default();
    let mut processed = HashSet::new();

    loop {
        if is_cancelled(cancel) {
            report.stop = IrManualStop::Cancelled;
            return Ok(report);
        }
        report.batches = report.batches.saturating_add(1);
        progress(IrManualProgress::FullUploadBatch { batch: report.batches });
        let backlog = network_db.ir_score_job_backlog_for_kind(
            &provider_key,
            &account_id,
            IrJobKind::Score,
            now_unix_seconds(),
            retry_filter,
        )?;
        if backlog.claimable > 0 {
            if options.resend {
                processed.extend(network_db.unfinished_ir_score_local_ids(
                    &provider_key,
                    &account_id,
                    IrJobKind::Score,
                )?);
            }
            progress(IrManualProgress::ExistingQueued {
                provider_key: provider_key.clone(),
                account_id: account_id.clone(),
                queued: backlog.claimable,
            });
            let outcome = sync_and_summarize(
                ctx,
                &mut network_db,
                &mut report,
                retry,
                Some(&mut breaker),
                progress,
                cancel,
            )
            .await?;
            if let Some(stop) = full_run_interruption(&outcome) {
                report.stop = stop;
                return Ok(report);
            }
            if outcome.report.submitted.saturating_add(outcome.report.failed) == 0
                && network_db
                    .ir_score_job_backlog_for_kind(
                        &provider_key,
                        &account_id,
                        IrJobKind::Score,
                        now_unix_seconds(),
                        retry_filter,
                    )?
                    .claimable
                    > 0
            {
                // claimできるはずのjobを取得できず進捗が無い。他の主体の処理を待つ。
                report.stop = IrManualStop::WaitingForSendingJob;
                return Ok(report);
            }
            // 数えた直後に別の主体が処理した場合も、次のbatchで残りを数え直す。
            continue;
        }

        let enqueue = enqueue_local_score_jobs_with_processed(
            ctx.sync.profile_root,
            ctx.sync.ir_config,
            &score_db,
            &library_db,
            &mut network_db,
            options,
            now_unix_seconds(),
            &mut processed,
        )?;
        absorb_enqueue(&mut report, &enqueue);
        let limit_reached = enqueue.limit_reached;
        let enqueued = enqueue.enqueued;
        progress(IrManualProgress::Enqueued {
            report: enqueue,
            dry_run: false,
            all: true,
            limit: options.limit.max(1),
        });
        if enqueued > 0 {
            if limit_reached {
                progress(IrManualProgress::LimitReachedContinuing);
            }
            continue;
        }

        // 新しい候補もclaimできるjobも無い。lease中のjobが残っていれば完了を待つ。
        report.remaining = network_db.unfinished_ir_score_job_count_for_kind(
            &provider_key,
            &account_id,
            IrJobKind::Score,
        )?;
        if backlog.leased > 0 {
            report.stop = IrManualStop::WaitingForSendingJob;
            return Ok(report);
        }
        progress(IrManualProgress::FullUploadComplete {
            enqueued: report.enqueued,
            submitted: report.submitted,
        });
        if report.failed > 0 {
            report.stop = IrManualStop::FailedJobs { failed: report.failed };
        }
        return Ok(report);
    }
}

/// `--all` の同期を途中で打ち切る理由。続行できるなら `None`。
fn full_run_interruption(outcome: &IrManualSyncOutcome) -> Option<IrManualStop> {
    if outcome.cancelled {
        return Some(IrManualStop::Cancelled);
    }
    outcome.stop
}
