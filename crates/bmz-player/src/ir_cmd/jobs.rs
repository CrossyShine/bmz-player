use super::output::{no_cancel, print_progress};
use super::*;
use crate::ir::manual::{IrManualSyncScope, IrSyncContext, sync_jobs};

pub(super) async fn sync_cli_jobs(
    network_db: &mut NetworkDatabase,
    score_db_path: &Path,
    profile_root: &Path,
    logs_dir: &Path,
    ir_config: &IrConfig,
    limit: u32,
) -> Result<IrSyncReport> {
    sync_cli_jobs_with_scope(
        network_db,
        score_db_path,
        profile_root,
        logs_dir,
        ir_config,
        limit,
        IrManualSyncScope::Any,
    )
    .await
}

pub(super) async fn sync_cli_jobs_for_kind(
    network_db: &mut NetworkDatabase,
    score_db_path: &Path,
    profile_root: &Path,
    logs_dir: &Path,
    ir_config: &IrConfig,
    provider_key: &str,
    account_id: &str,
    kind: IrJobKind,
    limit: u32,
) -> Result<IrSyncReport> {
    sync_cli_jobs_with_scope(
        network_db,
        score_db_path,
        profile_root,
        logs_dir,
        ir_config,
        limit,
        IrManualSyncScope::Kind { provider_key, account_id, kind },
    )
    .await
}

async fn sync_cli_jobs_with_scope(
    network_db: &mut NetworkDatabase,
    score_db_path: &Path,
    profile_root: &Path,
    logs_dir: &Path,
    ir_config: &IrConfig,
    limit: u32,
    scope: IrManualSyncScope<'_>,
) -> Result<IrSyncReport> {
    let ctx = IrSyncContext { score_db: score_db_path, profile_root, logs_dir, ir_config };
    let (_keep_alive, cancel) = no_cancel();
    let outcome =
        sync_jobs(&ctx, network_db, scope, limit, &mut |event| print_progress(&event), &cancel)
            .await?;
    Ok(outcome.report)
}
