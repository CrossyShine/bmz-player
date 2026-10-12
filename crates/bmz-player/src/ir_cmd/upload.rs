use super::download::now_unix_seconds;
use super::output::{no_cancel, print_progress};
use super::*;
use crate::ir::manual::{
    IrManualContext, IrManualStop, IrSyncContext, upload_local_all, upload_local_batch,
};

pub(super) async fn upload_local(
    app_paths: &AppPaths,
    profile_paths: &ProfilePaths,
    profile: &ProfileConfig,
    options: IrLocalUploadOptions,
    sync_after_enqueue: bool,
    all: bool,
) -> Result<()> {
    if options.dry_run && all {
        bail!("ir upload-local --all cannot be combined with --dry-run");
    }
    crate::storage::migration::migrate_library_db(&app_paths.library_db)?;
    crate::storage::migration::migrate_score_db(&profile_paths.score_db)?;
    crate::storage::migration::migrate_network_db(&profile_paths.network_db)?;

    let ctx = IrManualContext {
        library_db: &app_paths.library_db,
        network_db: &profile_paths.network_db,
        sync: IrSyncContext {
            score_db: &profile_paths.score_db,
            profile_root: profile_paths.root_dir.as_path(),
            logs_dir: app_paths.logs_dir.as_path(),
            ir_config: &profile.ir,
        },
    };
    let (_keep_alive, cancel) = no_cancel();
    let mut sink = |event| print_progress(&event);
    if !all {
        let report =
            upload_local_batch(&ctx, &options, sync_after_enqueue, &mut sink, &cancel).await?;
        if report.skipped_resend_payload_unavailable > 0 {
            bail!(
                "{} succeeded score(s) cannot be resent because their original request payload is unavailable",
                report.skipped_resend_payload_unavailable
            );
        }
        return Ok(());
    }

    let report = upload_local_all(&ctx, &options, now_unix_seconds(), &mut sink, &cancel).await?;
    if report.skipped_resend_payload_unavailable > 0 {
        bail!(
            "{} succeeded score(s) cannot be resent because their original request payload is unavailable",
            report.skipped_resend_payload_unavailable
        );
    }
    match report.stop {
        IrManualStop::FailedJobs { failed } => bail!(
            "full local upload finished with {} submissions and {failed} failed job(s); failed jobs stay queued for retry",
            report.submitted
        ),
        IrManualStop::ConsecutiveFailures { consecutive } => bail!(
            "full local upload stopped after {consecutive} consecutive failures ({} submitted); check the IR server and network, then rerun",
            report.submitted
        ),
        IrManualStop::Unauthorized => bail!(
            "full local upload stopped because the IR account is not authorized ({} submitted); run `bmz ir login` and rerun",
            report.submitted
        ),
        IrManualStop::RateLimited { retry_after_seconds } => bail!(
            "full local upload stopped because the IR server is rate limiting submissions ({} submitted); rerun{}",
            report.submitted,
            retry_after_seconds
                .map(|seconds| format!(" after {seconds} seconds"))
                .unwrap_or_else(|| " later".to_string())
        ),
        IrManualStop::WaitingForSendingJob => bail!(
            "full local upload is waiting for an existing sending job; rerun after its 5-minute lease expires"
        ),
        IrManualStop::Completed | IrManualStop::Cancelled => Ok(()),
    }
}
