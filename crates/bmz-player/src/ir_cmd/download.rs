use super::output::{no_cancel, print_progress};
use super::*;
use crate::ir::manual::{IrSyncContext, download_scores as download_scores_manual};

pub(super) async fn download_scores(
    app_paths: &AppPaths,
    profile_paths: &ProfilePaths,
    profile: &ProfileConfig,
    options: IrScoreDownloadOptions,
) -> Result<()> {
    crate::storage::migration::migrate_score_db(&profile_paths.score_db)?;
    crate::storage::migration::migrate_network_db(&profile_paths.network_db)?;

    let ctx = IrSyncContext {
        score_db: &profile_paths.score_db,
        profile_root: profile_paths.root_dir.as_path(),
        logs_dir: app_paths.logs_dir.as_path(),
        ir_config: &profile.ir,
    };
    let (_keep_alive, cancel) = no_cancel();
    let report = download_scores_manual(
        &ctx,
        &profile_paths.network_db,
        &options,
        &mut |event| print_progress(&event),
        &cancel,
    )
    .await?;

    println!("provider: {}", report.provider_key);
    println!("account: {}", report.account_id);
    println!("pages: {}", report.pages);
    println!("scanned: {}", report.scanned);
    if options.dry_run {
        println!("would import: {}", report.candidates);
        println!("would link existing uploads: {}", report.linked_existing);
    } else {
        println!("imported: {}", report.imported);
        println!("linked existing uploads: {}", report.linked_existing);
    }
    println!("skipped: existing={}, failed={}", report.skipped_existing, report.failed);
    if report.limit_reached {
        println!("limit reached; rerun `bmz ir download-scores` to continue");
    }
    for message in &report.messages {
        println!("  {message}");
    }
    ensure_download_report_success(&report)?;
    Ok(())
}

fn ensure_download_report_success(
    report: &crate::ir::download::IrScoreDownloadReport,
) -> Result<()> {
    if report.failed > 0 {
        bail!("IR score download had {} row failure(s)", report.failed);
    }
    Ok(())
}

pub(super) fn primary_provider(profile: &ProfileConfig) -> Result<&IrProviderConfig> {
    selected_provider(profile, None)
}

pub(super) fn selected_provider<'a>(
    profile: &'a ProfileConfig,
    requested: Option<&str>,
) -> Result<&'a IrProviderConfig> {
    crate::ir::manual::selected_provider(&profile.ir, requested)
}

pub(super) fn parse_scope(value: &str) -> Result<IrRankingScope> {
    Ok(match value {
        "global" => IrRankingScope::Global,
        "self_and_rivals" => IrRankingScope::SelfAndRivals,
        "rivals" => IrRankingScope::Rivals,
        "self" => IrRankingScope::SelfOnly,
        "around_self" => IrRankingScope::AroundSelf,
        other => bail!("unknown ranking scope: {other}"),
    })
}

pub(super) fn prompt_password() -> Result<String> {
    print!("password: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let password = line.trim_end_matches(['\r', '\n']).to_string();
    if password.is_empty() {
        bail!("password must not be empty");
    }
    Ok(password)
}

pub(super) fn now_unix_seconds() -> i64 {
    crate::ir::manual::now_unix_seconds()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_report_validator_returns_error_after_any_row_failure() {
        assert!(
            ensure_download_report_success(&crate::ir::download::IrScoreDownloadReport::default())
                .is_ok()
        );
        let report = crate::ir::download::IrScoreDownloadReport {
            failed: 1,
            imported: 2,
            ..Default::default()
        };
        let error = ensure_download_report_success(&report).unwrap_err();
        assert!(error.to_string().contains("1 row failure"));
    }
}
