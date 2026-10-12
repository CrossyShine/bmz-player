use std::collections::HashSet;

use anyhow::{Result, bail};

use crate::ir::backfill::{
    IrLocalUploadOptions, IrLocalUploadReport, enqueue_local_score_jobs,
    enqueue_local_score_jobs_with_processed, resolve_local_upload_target,
};
use crate::ir::sync::{IR_CLI_SYNC_BATCH_LIMIT, IrSyncReport};
use crate::storage::library_db::LibraryDatabase;
use crate::storage::network_db::{IrJobKind, NetworkDatabase};
use crate::storage::score_db::ScoreDatabase;

use super::sync::{IrManualSyncScope, sync_jobs};
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

/// 1回分のsyncを実行し、集計・remaining通知まで行う。中断されたら true。
async fn sync_and_summarize(
    ctx: &IrManualContext<'_>,
    network_db: &mut NetworkDatabase,
    report: &mut IrManualUploadReport,
    progress: IrManualProgressSink<'_>,
    cancel: &IrManualCancel,
) -> Result<(IrSyncReport, bool)> {
    let outcome = sync_jobs(
        &ctx.sync,
        network_db,
        IrManualSyncScope::Kind {
            provider_key: &report.provider_key,
            account_id: &report.account_id,
            kind: IrJobKind::Score,
        },
        IR_CLI_SYNC_BATCH_LIMIT,
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
    Ok((outcome.report, outcome.cancelled))
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
            let (sync_report, cancelled) =
                sync_and_summarize(ctx, &mut network_db, &mut report, progress, cancel).await?;
            if cancelled {
                report.stop = IrManualStop::Cancelled;
            } else if (sync_report.failed > 0 || report.remaining > 0)
                && let Some(stop) = full_upload_stop_reason(&sync_report)
            {
                report.stop = stop;
            }
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
    let (sync_report, cancelled) =
        sync_and_summarize(ctx, &mut network_db, &mut report, progress, cancel).await?;
    if cancelled {
        report.stop = IrManualStop::Cancelled;
    } else if (sync_report.failed > 0 || report.remaining > 0)
        && let Some(stop) = full_upload_stop_reason(&sync_report)
    {
        report.stop = stop;
    }
    Ok(report)
}

/// `bmz ir upload-local --all` 相当。未完了jobの同期とenqueueを、候補が無くなるまで繰り返す。
///
/// 失敗jobが出た場合と、他のsending jobしか残らず進捗が無い場合は、
/// それぞれ `IrManualStop::FailedJobs` / `WaitingForSendingJob` で停止する。
pub async fn upload_local_all(
    ctx: &IrManualContext<'_>,
    options: &IrLocalUploadOptions,
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
    let mut processed = HashSet::new();

    loop {
        if is_cancelled(cancel) {
            report.stop = IrManualStop::Cancelled;
            return Ok(report);
        }
        report.batches = report.batches.saturating_add(1);
        progress(IrManualProgress::FullUploadBatch { batch: report.batches });
        let queued = network_db.unfinished_ir_score_job_count_for_kind(
            &provider_key,
            &account_id,
            IrJobKind::Score,
        )?;
        if queued > 0 {
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
                queued,
            });
            let (sync_report, cancelled) =
                sync_and_summarize(ctx, &mut network_db, &mut report, progress, cancel).await?;
            if cancelled {
                report.stop = IrManualStop::Cancelled;
                return Ok(report);
            }
            if (sync_report.failed > 0 || report.remaining > 0)
                && let Some(stop) = full_upload_stop_reason(&sync_report)
            {
                report.stop = stop;
                return Ok(report);
            }
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
        if enqueued == 0 {
            progress(IrManualProgress::FullUploadComplete {
                enqueued: report.enqueued,
                submitted: report.submitted,
            });
            return Ok(report);
        }
        if limit_reached {
            progress(IrManualProgress::LimitReachedContinuing);
        }

        let (sync_report, cancelled) =
            sync_and_summarize(ctx, &mut network_db, &mut report, progress, cancel).await?;
        if cancelled {
            report.stop = IrManualStop::Cancelled;
            return Ok(report);
        }
        if (sync_report.failed > 0 || report.remaining > 0)
            && let Some(stop) = full_upload_stop_reason(&sync_report)
        {
            report.stop = stop;
            return Ok(report);
        }
        if report.remaining == 0 && !limit_reached {
            progress(IrManualProgress::FullUploadComplete {
                enqueued: report.enqueued,
                submitted: report.submitted,
            });
            return Ok(report);
        }
    }
}
