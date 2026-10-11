/// IR ジョブの種別。単曲スコア、コーススコア、リプレイ、または既送信scoreへの署名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrJobKind {
    Score,
    Course,
    Replay,
    Attestation,
}

impl IrJobKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Score => "score",
            Self::Course => "course",
            Self::Replay => "replay",
            Self::Attestation => "attestation",
        }
    }

    pub fn from_str_or_score(value: &str) -> Self {
        match value {
            "course" => Self::Course,
            "replay" => Self::Replay,
            "attestation" => Self::Attestation,
            _ => Self::Score,
        }
    }
}

impl IrScoreJobStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sending => "sending",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IrScoreJobRecord {
    pub id: i64,
    pub provider: String,
    pub account_id: String,
    pub kind: IrJobKind,
    pub local_score_id: i64,
    pub chart_sha256: [u8; 32],
    pub ln_policy: LnScorePolicy,
    pub payload_json: String,
    pub status: String,
    pub attempt_count: u32,
    pub next_attempt_at: i64,
    pub last_error: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 送信キュー一覧用の軽量レコード。重い `payload_json` は含めない。
#[derive(Debug, Clone)]
pub struct IrScoreJobSummary {
    pub id: i64,
    pub provider: String,
    pub account_id: String,
    pub kind: IrJobKind,
    pub local_score_id: i64,
    pub chart_sha256: [u8; 32],
    pub ln_policy: LnScorePolicy,
    pub status: String,
    pub attempt_count: u32,
    pub next_attempt_at: i64,
    pub last_error: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// `list_ir_score_jobs` / `count_ir_score_jobs` の絞り込み条件。
#[derive(Debug, Clone, Copy)]
pub struct IrScoreJobListFilter<'a> {
    pub provider: Option<&'a str>,
    pub account_id: Option<&'a str>,
    /// 空なら全kind。
    pub kinds: &'a [IrJobKind],
    /// 空なら `succeeded` 以外（キューに残っているもの）。
    pub statuses: &'a [IrScoreJobStatus],
    pub limit: u32,
    pub offset: u32,
}

impl Default for IrScoreJobListFilter<'_> {
    fn default() -> Self {
        Self { provider: None, account_id: None, kinds: &[], statuses: &[], limit: 100, offset: 0 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IrScoreJobStatusCounts {
    pub pending: u32,
    pub sending: u32,
    pub failed: u32,
    pub succeeded: u32,
}

#[derive(Debug, Clone)]
pub struct NewIrScoreJob {
    pub provider: String,
    pub account_id: String,
    pub kind: IrJobKind,
    pub local_score_id: i64,
    pub chart_sha256: [u8; 32],
    pub ln_policy: LnScorePolicy,
    pub payload_json: String,
    pub now: i64,
}

#[derive(Debug, Clone)]
pub struct NewIrScoreSubmission {
    pub job_id: i64,
    pub provider: String,
    pub account_id: String,
    pub kind: IrJobKind,
    pub local_score_id: i64,
    pub remote_score_id: String,
    pub status: String,
    pub submitted_at: i64,
    pub log_path: String,
    pub error: String,
}

/// ローカル score_history と対応する、受理済み IR score の記録。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrSubmittedScoreLink {
    pub provider: String,
    pub account_id: String,
    pub local_score_id: i64,
    pub remote_score_id: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IrLocalScoreCleanupReport {
    pub removed_jobs: u32,
    pub removed_submissions: u32,
}

/// rianIR の軽量ライバル全曲APIから取得した、譜面/LNモード単位のベスト。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrRivalScoreRecord {
    pub chart_sha256: [u8; 32],
    pub ln_mode: u8,
    pub ex_score: u32,
    pub clear_type: i32,
    pub max_combo: u32,
    pub min_bp: i32,
    pub play_option: i32,
    pub arrange_1p: String,
    pub arrange_2p: String,
    pub double_option: String,
    pub play_seed: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IrRivalScoreCacheState {
    pub etag: String,
    pub fetched_at: i64,
}
use super::*;
