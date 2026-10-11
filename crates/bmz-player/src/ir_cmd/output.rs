//! `ir::manual` の進捗通知を従来のCLI出力へ変換する。
use super::*;
use crate::ir::manual::{IrManualCancel, IrManualProgress};

/// CLIは中断しない。送信側を返すのは、破棄されても待機が壊れないことに頼らないため。
pub(super) fn no_cancel() -> (tokio::sync::watch::Sender<bool>, IrManualCancel) {
    tokio::sync::watch::channel(false)
}

pub(super) fn print_progress(event: &IrManualProgress) {
    match event {
        IrManualProgress::FullUploadBatch { batch } => println!("full upload batch: {batch}"),
        IrManualProgress::ExistingQueued { provider_key, account_id, queued } => {
            println!("provider: {provider_key}");
            println!("account: {account_id}");
            println!("existing queued score jobs: {queued}; syncing before enqueueing more");
        }
        IrManualProgress::Enqueued { report, dry_run, all, limit } => {
            println!("provider: {}", report.provider_key);
            println!("account: {}", report.account_id);
            println!("scanned: {}", report.scanned);
            println!("candidates: {}", report.candidates);
            if *dry_run {
                println!("would enqueue: {}", report.candidates.min(*limit));
            } else {
                println!("enqueued: {}", report.enqueued);
            }
            println!(
                "skipped: already_submitted={}, already_queued={}, missing_chart={}, course_stage={}, autoplay={}",
                report.skipped_already_submitted,
                report.skipped_already_queued,
                report.skipped_missing_chart,
                report.skipped_course_stage,
                report.skipped_autoplay,
            );
            if report.missing_replays > 0 {
                println!(
                    "replay files missing: {} (scores were queued without replay)",
                    report.missing_replays
                );
            }
            if !*all && report.limit_reached {
                println!("limit reached; rerun `bmz ir upload-local` to enqueue the next batch");
            }
        }
        IrManualProgress::LimitReachedContinuing => {
            println!("limit reached; continuing because --all is enabled");
        }
        IrManualProgress::SyncStarted { limit, estimated_seconds } => {
            println!("syncing up to {limit} queued jobs (about {estimated_seconds}s)");
        }
        IrManualProgress::JobSyncing { index, limit } => {
            print!("[{index}/{limit}] syncing...");
            let _ = std::io::stdout().flush();
        }
        IrManualProgress::JobFinished { submitted, failed } => {
            println!(" submitted={submitted}, failed={failed}");
        }
        IrManualProgress::JobError => println!(" error"),
        IrManualProgress::NoQueuedJobs => println!(" no queued jobs"),
        IrManualProgress::SyncSummary {
            provider_key,
            account_id,
            submitted,
            failed,
            remaining,
            messages,
        } => {
            println!("submitted: {submitted}, failed: {failed}");
            println!("remaining queued score jobs for {provider_key}/{account_id}: {remaining}");
            for message in messages {
                println!("  {message}");
            }
        }
        IrManualProgress::FullUploadComplete { enqueued, submitted } => {
            println!("full local upload complete: enqueued={enqueued}, submitted={submitted}");
        }
        IrManualProgress::DownloadStarted { .. } | IrManualProgress::DownloadPage(_) => {}
    }
}
