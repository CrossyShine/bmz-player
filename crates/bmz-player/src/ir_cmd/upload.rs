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
        upload_local_batch(&ctx, &options, sync_after_enqueue, &mut sink, &cancel).await?;
        return Ok(());
    }

    let report = upload_local_all(&ctx, &options, &mut sink, &cancel).await?;
    match report.stop {
        IrManualStop::FailedJobs { failed } => bail!(
            "full local upload stopped after {} submissions because {failed} jobs failed",
            report.submitted
        ),
        IrManualStop::WaitingForSendingJob => bail!(
            "full local upload is waiting for an existing sending job; rerun after its 5-minute lease expires"
        ),
        IrManualStop::Completed | IrManualStop::Cancelled => Ok(()),
    }
}
