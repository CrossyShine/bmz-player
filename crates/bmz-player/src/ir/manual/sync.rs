use anyhow::Result;

use crate::ir::sync::{
    IR_CLI_SYNC_JOB_SPACING_MS, IrSyncJobFilter, IrSyncReport, IrSyncThrottle,
    sync_pending_ir_jobs, sync_pending_ir_jobs_filtered,
};
use crate::storage::network_db::{IrJobKind, NetworkDatabase};

use super::{
    IrManualCancel, IrManualProgress, IrManualProgressSink, IrSyncContext, is_cancelled,
    now_unix_seconds, sleep_or_cancel,
};

#[derive(Debug, Clone, Copy)]
pub enum IrManualSyncScope<'a> {
    /// 全provider・全kindのjob。
    Any,
    Kind {
        provider_key: &'a str,
        account_id: &'a str,
        kind: IrJobKind,
    },
}

#[derive(Debug, Default)]
pub struct IrManualSyncOutcome {
    pub report: IrSyncReport,
    pub cancelled: bool,
}

/// 未送信jobを1件ずつclaimして最大 `limit` 件送る。
///
/// 送信間隔は `IR_CLI_SYNC_JOB_SPACING_MS`。失敗が出たら以降は再試行待ち時刻を尊重する
/// （同じ失敗jobを連打しない）。
pub async fn sync_jobs(
    ctx: &IrSyncContext<'_>,
    network_db: &mut NetworkDatabase,
    scope: IrManualSyncScope<'_>,
    limit: u32,
    progress: IrManualProgressSink<'_>,
    cancel: &IrManualCancel,
) -> Result<IrManualSyncOutcome> {
    let estimated_seconds = u64::from(limit.saturating_sub(1))
        .saturating_mul(IR_CLI_SYNC_JOB_SPACING_MS)
        .div_ceil(1_000);
    progress(IrManualProgress::SyncStarted { limit, estimated_seconds });

    let mut outcome = IrManualSyncOutcome::default();
    for index in 0..limit {
        if is_cancelled(cancel) {
            outcome.cancelled = true;
            break;
        }
        let ignore_retry_backoff = outcome.report.failed == 0;
        if index > 0 {
            let now = now_unix_seconds();
            let has_next = match scope {
                IrManualSyncScope::Kind { provider_key, account_id, kind } => !network_db
                    .pending_ir_score_jobs_for_kind(
                        provider_key,
                        account_id,
                        kind,
                        now,
                        1,
                        ignore_retry_backoff,
                    )?
                    .is_empty(),
                IrManualSyncScope::Any if ignore_retry_backoff => {
                    !network_db.pending_ir_score_jobs_ignoring_backoff(now, 1)?.is_empty()
                }
                IrManualSyncScope::Any => !network_db.pending_ir_score_jobs(now, 1)?.is_empty(),
            };
            if !has_next {
                break;
            }
            if sleep_or_cancel(std::time::Duration::from_millis(IR_CLI_SYNC_JOB_SPACING_MS), cancel)
                .await
            {
                outcome.cancelled = true;
                break;
            }
        }
        progress(IrManualProgress::JobSyncing { index: index + 1, limit });
        let sync_result = match scope {
            IrManualSyncScope::Kind { provider_key, account_id, kind } => {
                sync_pending_ir_jobs_filtered(
                    network_db,
                    ctx.score_db,
                    ctx.profile_root,
                    ctx.logs_dir,
                    ctx.ir_config,
                    IrSyncJobFilter { provider_key, account_id, kind, local_score_id: None },
                    now_unix_seconds(),
                    1,
                    ignore_retry_backoff,
                    IrSyncThrottle::none(),
                )
                .await
            }
            IrManualSyncScope::Any => {
                sync_pending_ir_jobs(
                    network_db,
                    ctx.score_db,
                    ctx.profile_root,
                    ctx.logs_dir,
                    ctx.ir_config,
                    now_unix_seconds(),
                    1,
                    ignore_retry_backoff,
                    IrSyncThrottle::none(),
                )
                .await
            }
        };
        let report = match sync_result {
            Ok(report) => report,
            Err(error) => {
                progress(IrManualProgress::JobError);
                return Err(error);
            }
        };
        if report.submitted.saturating_add(report.failed) == 0 {
            progress(IrManualProgress::NoQueuedJobs);
            break;
        }
        progress(IrManualProgress::JobFinished {
            submitted: report.submitted,
            failed: report.failed,
        });
        let total = &mut outcome.report;
        total.submitted = total.submitted.saturating_add(report.submitted);
        total.failed = total.failed.saturating_add(report.failed);
        total.messages.extend(report.messages);
        total.included_rankings.extend(report.included_rankings);
    }
    Ok(outcome)
}
