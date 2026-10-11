use super::*;
use crate::ir::manual_job::{IrManualEnv, IrManualOp, IrManualOutcome, IrManualOutcomeStatus};

impl WinitApp {
    fn ir_manual_env(&self) -> IrManualEnv {
        IrManualEnv {
            library_db: self.boot.app_paths.library_db.clone(),
            score_db: self.boot.profile_paths.score_db.clone(),
            network_db: self.boot.profile_paths.network_db.clone(),
            profile_root: self.boot.profile_paths.root_dir.clone(),
            logs_dir: self.boot.app_paths.logs_dir.clone(),
            ir_config: self.boot.profile_config.ir.clone(),
        }
    }

    /// UIが積んだ手動IR要求を開始し、完了した操作をtoastで通知する。
    pub(in crate::app) fn poll_ir_manual(&mut self) {
        for queued in self.jobs.ir_manual.drain_requests() {
            self.jobs.ir_manual.dispatch(self.ir_manual_env(), queued);
        }
        if let Some(outcome) = self.jobs.ir_manual.take_notice() {
            let message =
                ir_manual_notice(Localizer::new(self.boot.profile_config.ui.locale()), &outcome);
            tracing::info!(op = ?outcome.op, status = ?outcome.status, "manual IR operation finished");
            self.show_left_overlay_toast(message);
        }
    }
}

pub(super) fn ir_manual_notice(text: Localizer, outcome: &IrManualOutcome) -> String {
    let progress = &outcome.progress;
    let resend =
        matches!(outcome.op, IrManualOp::ResendJob { .. } | IrManualOp::ResendFailed { .. });
    let mut args = FluentArgs::new();
    match &outcome.status {
        IrManualOutcomeStatus::Cancelled => text.text("toast-ir-manual-cancelled"),
        IrManualOutcomeStatus::Error(error) => {
            args.set("error", error.clone());
            text.format("toast-ir-manual-error", &args)
        }
        IrManualOutcomeStatus::WaitingTimedOut => text.text("toast-ir-manual-waiting-timeout"),
        IrManualOutcomeStatus::FailedJobs { failed } if !resend => {
            args.set("failed", i64::from(*failed));
            text.format("toast-ir-manual-failed-jobs", &args)
        }
        IrManualOutcomeStatus::Completed | IrManualOutcomeStatus::FailedJobs { .. } => {
            match outcome.op {
                IrManualOp::UploadLocalAll => {
                    args.set("submitted", i64::from(progress.submitted));
                    args.set("failed", i64::from(progress.failed));
                    text.format("toast-ir-upload-completed", &args)
                }
                IrManualOp::DownloadScores => {
                    args.set("imported", i64::from(progress.download_imported));
                    args.set("linked", i64::from(progress.download_linked));
                    text.format("toast-ir-download-completed", &args)
                }
                IrManualOp::ResendJob { .. } | IrManualOp::ResendFailed { .. } => {
                    args.set("submitted", i64::from(progress.submitted));
                    args.set("failed", i64::from(progress.failed));
                    text.format("toast-ir-resend-completed", &args)
                }
            }
        }
    }
}
