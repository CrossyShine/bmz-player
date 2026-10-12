use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, params_from_iter};

use super::common::{configure_connection, hash_to_hex, hex_to_hash};
use crate::ln_policy::LnScorePolicy;

const SUCCEEDED_IR_SCORE_JOB_RETENTION_SECONDS: i64 = 30 * 24 * 60 * 60;
const SUCCEEDED_IR_SCORE_JOB_RETAIN_RECENT_COUNT: u32 = 500;

pub struct NetworkDatabase {
    conn: Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrScoreJobStatus {
    Pending,
    Sending,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBackfillResendOutcome {
    Enqueued,
    AlreadyPending,
    Busy,
    PayloadUnavailable,
    NeedsPayload,
}

mod model;
mod operations;
mod rows;

pub use model::{
    IrJobKind, IrLocalScoreCleanupReport, IrRivalScoreCacheState, IrRivalScoreRecord,
    IrScoreJobListFilter, IrScoreJobRecord, IrScoreJobStatusCounts, IrScoreJobSummary,
    IrSubmittedScoreLink, NewIrScoreJob, NewIrScoreSubmission,
};

#[cfg(test)]
#[path = "network_db/tests.rs"]
mod tests;
