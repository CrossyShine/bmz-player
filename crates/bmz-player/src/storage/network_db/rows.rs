use super::*;

pub(super) fn sql_placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count).collect::<Vec<_>>().join(", ")
}

pub(super) fn ir_score_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IrScoreJobRecord> {
    let chart_sha256: String = row.get(4)?;
    let kind: String = row.get(13)?;
    Ok(IrScoreJobRecord {
        id: row.get(0)?,
        provider: row.get(1)?,
        account_id: row.get(2)?,
        kind: IrJobKind::from_str_or_score(&kind),
        local_score_id: row.get(3)?,
        chart_sha256: hex_to_hash(&chart_sha256)?,
        ln_policy: ln_policy_from_row(row, 5)?,
        payload_json: row.get(6)?,
        status: row.get(7)?,
        attempt_count: row.get(8)?,
        next_attempt_at: row.get(9)?,
        last_error: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

/// `ir_score_job_summary_columns` の並びに対応する。
pub(super) fn ir_score_job_summary_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<IrScoreJobSummary> {
    let chart_sha256: String = row.get(4)?;
    let kind: String = row.get(12)?;
    Ok(IrScoreJobSummary {
        id: row.get(0)?,
        provider: row.get(1)?,
        account_id: row.get(2)?,
        kind: IrJobKind::from_str_or_score(&kind),
        local_score_id: row.get(3)?,
        chart_sha256: hex_to_hash(&chart_sha256)?,
        ln_policy: ln_policy_from_row(row, 5)?,
        status: row.get(6)?,
        attempt_count: row.get(7)?,
        next_attempt_at: row.get(8)?,
        last_error: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn ln_policy_from_row(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<LnScorePolicy> {
    let value: String = row.get(index)?;
    LnScorePolicy::from_str_opt(&value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            format!("invalid LN score policy: {value}").into(),
        )
    })
}
