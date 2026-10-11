use super::*;

pub(super) fn ln_policy_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<LnScorePolicy> {
    let value: String = row.get(index)?;
    LnScorePolicy::from_str_opt(&value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            format!("invalid LN score policy: {value}").into(),
        )
    })
}

pub(super) fn double_option_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<DoubleOptionScoreBucket> {
    let value: String = row.get(index)?;
    Ok(DoubleOptionScoreBucket::from_str_or_off(&value))
}

pub(super) fn applied_double_option_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<DoubleOption> {
    let value: String = row.get(index)?;
    Ok(DoubleOption::from_persistent_str(&value))
}

pub(super) fn rule_mode_from_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<RuleMode> {
    let value: String = row.get(index)?;
    RuleMode::from_str_opt(&value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            format!("invalid rule mode: {value}").into(),
        )
    })
}

pub(super) fn previous_best_snapshot(
    conn: &Connection,
    key: ScoreKey,
) -> Result<Option<PreviousBestSnapshot>> {
    conn.query_row(
        "SELECT clear_type, ex_score, max_combo, bp, cb
         FROM score_best
         WHERE chart_sha256 = ?1 AND ln_policy = ?2 AND double_option = ?3
           AND rule_mode = ?4",
        params![
            hash_to_hex(&key.chart_sha256),
            key.ln_policy.as_str(),
            key.double_option.as_str(),
            key.rule_mode.as_str(),
        ],
        |row| {
            Ok(PreviousBestSnapshot {
                clear_type: row.get(0)?,
                ex_score: row.get(1)?,
                max_combo: row.get(2)?,
                bp: row.get(3)?,
                cb: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub(super) fn insert_score_history(
    conn: &Connection,
    record: &ScoreRecord,
    previous_best: Option<&PreviousBestSnapshot>,
) -> Result<()> {
    let judges = &record.score.judges;
    let bp = score_record_bp(record);
    let cb = score_record_cb(record);
    conn.execute(
        "INSERT INTO score_history (
            chart_sha256,
            ln_policy,
            double_option,
            played_at,
            clear_type,
            gauge_type,
            gauge_value,
            total_notes,
            ex_score,
            bp,
            cb,
            max_combo,
            fast_pgreat,
            slow_pgreat,
            fast_great,
            slow_great,
            fast_good,
            slow_good,
            fast_bad,
            slow_bad,
            fast_poor,
            slow_poor,
            fast_empty_poor,
            slow_empty_poor,
            random_seed,
            arrange,
            arrange_2p,
            gauge_option,
            rule_mode,
            assist_mask,
            autoplay,
            device_type,
            replay_path,
            source_kind,
            applied_double_option,
            old_clear_type,
            old_ex_score,
            old_max_combo,
            old_bp,
            old_cb,
            seed_scheme
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30,
            ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41
        )",
        params![
            hash_to_hex(&record.chart_sha256),
            record.ln_policy.as_str(),
            record.double_option.as_str(),
            record.played_at,
            record.clear_type.as_str(),
            gauge_type_str(record.gauge_type),
            record.gauge_value,
            record.total_notes,
            record.score.ex_score(),
            bp,
            cb,
            record.score.max_combo,
            judges.fast_pgreat,
            judges.slow_pgreat,
            judges.fast_great,
            judges.slow_great,
            judges.fast_good,
            judges.slow_good,
            judges.fast_bad,
            judges.slow_bad,
            judges.fast_poor,
            judges.slow_poor,
            judges.fast_empty_poor,
            judges.slow_empty_poor,
            record.random_seed,
            record.arrange.as_str(),
            record.arrange_2p.as_str(),
            record.gauge_option.as_str(),
            record.rule_mode.as_str(),
            record.assist_mask,
            record.autoplay,
            record.device_type.as_str(),
            record.replay_path.as_str(),
            record.source_kind.as_str(),
            record.applied_double_option.to_persistent_str(),
            previous_best.map(|best| best.clear_type.as_str()),
            previous_best.map(|best| best.ex_score),
            previous_best.map(|best| best.max_combo),
            previous_best.map(|best| best.bp),
            previous_best.map(|best| best.cb),
            record.seed_scheme.as_str(),
        ],
    )?;
    Ok(())
}

pub(super) fn insert_score_history_ir_identity(
    conn: &Connection,
    score_history_id: i64,
) -> Result<()> {
    let idempotency_key = crate::ir::payload::new_score_idempotency_key()?;
    conn.execute(
        "INSERT INTO score_history_ir_identity (score_history_id, idempotency_key)
         VALUES (?1, ?2)",
        params![score_history_id, idempotency_key],
    )?;
    Ok(())
}

pub(super) fn score_history_id_for_source(
    conn: &Connection,
    key: &ScoreHistorySourceKey,
) -> Result<Option<i64>> {
    conn.query_row(
        "SELECT score_history_id
         FROM score_history_sources
         WHERE source = ?1
           AND provider = ?2
           AND account_id = ?3
           AND remote_score_id = ?4",
        params![
            key.source.as_str(),
            key.provider.as_str(),
            key.account_id.as_str(),
            key.remote_score_id.as_str(),
        ],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub(super) fn insert_score_history_source(
    conn: &Connection,
    score_history_id: i64,
    source: &ScoreHistorySourceRecord,
    ignore_duplicate: bool,
) -> Result<usize> {
    let insert = if ignore_duplicate { "INSERT OR IGNORE" } else { "INSERT" };
    let sql = format!(
        "{insert} INTO score_history_sources (
            score_history_id, source, provider, account_id, remote_score_id,
            verification, server_received_at, imported_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
    );
    conn.execute(
        &sql,
        params![
            score_history_id,
            source.key.source.as_str(),
            source.key.provider.as_str(),
            source.key.account_id.as_str(),
            source.key.remote_score_id.as_str(),
            source.verification.as_str(),
            source.server_received_at,
            source.imported_at,
        ],
    )
    .map_err(Into::into)
}

pub(super) fn record_rule_mode(record: &ScoreRecord) -> RuleMode {
    RuleMode::from_str_opt(&record.rule_mode).unwrap_or(RuleMode::Beatoraja)
}

pub(super) fn update_player_stats(conn: &Connection, record: &ScoreRecord) -> Result<()> {
    let judges = &record.score.judges;
    let clear_increment = u32::from(is_counted_clear(record.clear_type));
    conn.execute(
        "INSERT INTO player_stats (
            id,
            play_count,
            clear_count,
            playtime_seconds,
            max_combo,
            fast_pgreat,
            slow_pgreat,
            fast_great,
            slow_great,
            fast_good,
            slow_good,
            fast_bad,
            slow_bad,
            fast_poor,
            slow_poor,
            fast_empty_poor,
            slow_empty_poor,
            updated_at
        ) VALUES (
            1, 1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
        )
        ON CONFLICT(id) DO UPDATE SET
            play_count = play_count + 1,
            clear_count = clear_count + excluded.clear_count,
            playtime_seconds = playtime_seconds + excluded.playtime_seconds,
            max_combo = max(max_combo, excluded.max_combo),
            fast_pgreat = fast_pgreat + excluded.fast_pgreat,
            slow_pgreat = slow_pgreat + excluded.slow_pgreat,
            fast_great = fast_great + excluded.fast_great,
            slow_great = slow_great + excluded.slow_great,
            fast_good = fast_good + excluded.fast_good,
            slow_good = slow_good + excluded.slow_good,
            fast_bad = fast_bad + excluded.fast_bad,
            slow_bad = slow_bad + excluded.slow_bad,
            fast_poor = fast_poor + excluded.fast_poor,
            slow_poor = slow_poor + excluded.slow_poor,
            fast_empty_poor = fast_empty_poor + excluded.fast_empty_poor,
            slow_empty_poor = slow_empty_poor + excluded.slow_empty_poor,
            updated_at = max(updated_at, excluded.updated_at)",
        params![
            clear_increment,
            record.playtime_seconds,
            record.score.max_combo,
            judges.fast_pgreat,
            judges.slow_pgreat,
            judges.fast_great,
            judges.slow_great,
            judges.fast_good,
            judges.slow_good,
            judges.fast_bad,
            judges.slow_bad,
            judges.fast_poor,
            judges.slow_poor,
            judges.fast_empty_poor,
            judges.slow_empty_poor,
            record.played_at,
        ],
    )?;
    Ok(())
}

pub(super) fn upsert_score_best(
    conn: &Connection,
    record: &ScoreRecord,
    history_id: i64,
) -> Result<()> {
    let judges = &record.score.judges;
    let ghost = encode_beatoraja_ghost(&record.score.ghost)?;
    let clear_increment = u32::from(is_counted_clear(record.clear_type));
    let bp = score_record_bp(record);
    let cb = score_record_cb(record);
    let rule_mode = record_rule_mode(record);
    let inserted = conn.execute(
        "INSERT INTO score_best (
            chart_sha256,
            ln_policy,
            double_option,
            rule_mode,
            clear_type,
            gauge_type,
            gauge_value,
            ex_score,
            bp,
            cb,
            max_combo,
            fast_pgreat,
            slow_pgreat,
            fast_great,
            slow_great,
            fast_good,
            slow_good,
            fast_bad,
            slow_bad,
            fast_poor,
            slow_poor,
            fast_empty_poor,
            slow_empty_poor,
            played_at,
            replay_path,
            ghost,
            device_type,
            best_score_history_id,
            play_count,
            clear_count
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30
        )
        ON CONFLICT(chart_sha256, ln_policy, double_option, rule_mode) DO NOTHING",
        params![
            hash_to_hex(&record.chart_sha256),
            record.ln_policy.as_str(),
            record.double_option.as_str(),
            rule_mode.as_str(),
            record.clear_type.as_str(),
            gauge_type_str(record.gauge_type),
            record.gauge_value,
            record.score.ex_score(),
            bp,
            cb,
            record.score.max_combo,
            judges.fast_pgreat,
            judges.slow_pgreat,
            judges.fast_great,
            judges.slow_great,
            judges.fast_good,
            judges.slow_good,
            judges.fast_bad,
            judges.slow_bad,
            judges.fast_poor,
            judges.slow_poor,
            judges.fast_empty_poor,
            judges.slow_empty_poor,
            record.played_at,
            record.replay_path.as_str(),
            ghost,
            record.device_type.as_str(),
            history_id,
            1_u32,
            clear_increment,
        ],
    )?;
    if inserted > 0 {
        return Ok(());
    }

    let chart_sha256 = hash_to_hex(&record.chart_sha256);
    conn.execute(
        "UPDATE score_best
         SET play_count = play_count + 1,
             clear_count = clear_count + ?2
         WHERE chart_sha256 = ?1 AND ln_policy = ?3 AND double_option = ?4
           AND rule_mode = ?5",
        params![
            chart_sha256,
            clear_increment,
            record.ln_policy.as_str(),
            record.double_option.as_str(),
            rule_mode.as_str(),
        ],
    )?;

    let current = conn.query_row(
        "SELECT ex_score, clear_type, bp, cb, max_combo
         FROM score_best
         WHERE chart_sha256 = ?1 AND ln_policy = ?2 AND double_option = ?3
           AND rule_mode = ?4",
        params![
            hash_to_hex(&record.chart_sha256),
            record.ln_policy.as_str(),
            record.double_option.as_str(),
            rule_mode.as_str(),
        ],
        |row| {
            let clear_type: String = row.get(1)?;
            Ok(ScoreBestRank {
                ex_score: row.get(0)?,
                clear_rank: clear_rank_from_name(&clear_type),
                bp: row.get(2)?,
                cb: row.get(3)?,
                max_combo: row.get(4)?,
            })
        },
    )?;
    let should_update_score = score_best_should_update_score(record, current);
    let should_update_clear = score_best_should_update_clear(record, current);
    if !should_update_score {
        conn.execute(
            "UPDATE score_best SET
                bp = min(bp, ?2),
                cb = min(cb, ?3),
                max_combo = max(max_combo, ?4)
             WHERE chart_sha256 = ?1 AND ln_policy = ?5 AND double_option = ?6
               AND rule_mode = ?7",
            params![
                hash_to_hex(&record.chart_sha256),
                bp,
                cb,
                record.score.max_combo,
                record.ln_policy.as_str(),
                record.double_option.as_str(),
                rule_mode.as_str(),
            ],
        )?;
    } else {
        conn.execute(
            "UPDATE score_best SET
                ex_score = ?2,
                bp = ?3,
                cb = ?4,
                max_combo = ?5,
                fast_pgreat = ?6,
                slow_pgreat = ?7,
                fast_great = ?8,
                slow_great = ?9,
                fast_good = ?10,
                slow_good = ?11,
                fast_bad = ?12,
                slow_bad = ?13,
                fast_poor = ?14,
                slow_poor = ?15,
                fast_empty_poor = ?16,
                slow_empty_poor = ?17,
                played_at = ?18,
                replay_path = ?19,
                ghost = ?20,
                device_type = ?21,
                best_score_history_id = ?22
             WHERE chart_sha256 = ?1 AND ln_policy = ?23 AND double_option = ?24
               AND rule_mode = ?25",
            params![
                hash_to_hex(&record.chart_sha256),
                record.score.ex_score(),
                bp,
                cb,
                record.score.max_combo,
                judges.fast_pgreat,
                judges.slow_pgreat,
                judges.fast_great,
                judges.slow_great,
                judges.fast_good,
                judges.slow_good,
                judges.fast_bad,
                judges.slow_bad,
                judges.fast_poor,
                judges.slow_poor,
                judges.fast_empty_poor,
                judges.slow_empty_poor,
                record.played_at,
                record.replay_path.as_str(),
                ghost,
                record.device_type.as_str(),
                history_id,
                record.ln_policy.as_str(),
                record.double_option.as_str(),
                rule_mode.as_str(),
            ],
        )?;
    }

    if should_update_clear {
        conn.execute(
            "UPDATE score_best SET
                clear_type = ?2,
                gauge_type = ?3,
                gauge_value = ?4
             WHERE chart_sha256 = ?1 AND ln_policy = ?5 AND double_option = ?6
               AND rule_mode = ?7",
            params![
                hash_to_hex(&record.chart_sha256),
                record.clear_type.as_str(),
                gauge_type_str(record.gauge_type),
                record.gauge_value,
                record.ln_policy.as_str(),
                record.double_option.as_str(),
                rule_mode.as_str(),
            ],
        )?;
    }
    Ok(())
}

/// 数値スコアを無効化したプレイについて、ランプと回数だけを更新する。
///
/// 新規キーには中立値の score_best 行を作る。後から通常プレイが記録されると
/// [`upsert_score_best`] が数値項目と `best_score_history_id` を通常どおり埋める。
pub(super) fn upsert_score_best_clear_only(conn: &Connection, record: &ScoreRecord) -> Result<()> {
    let clear_increment = u32::from(is_counted_clear(record.clear_type));
    let rule_mode = record_rule_mode(record);
    let chart_sha256 = hash_to_hex(&record.chart_sha256);
    let inserted = conn.execute(
        "INSERT INTO score_best (
            chart_sha256, ln_policy, double_option, rule_mode,
            clear_type, gauge_type, gauge_value,
            ex_score, bp, cb, max_combo,
            fast_pgreat, slow_pgreat, fast_great, slow_great,
            fast_good, slow_good, fast_bad, slow_bad,
            fast_poor, slow_poor, fast_empty_poor, slow_empty_poor,
            played_at, replay_path, ghost, device_type,
            best_score_history_id, play_count, clear_count
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            ?8, '', '', ?9,
            NULL, 1, ?10
        )
        ON CONFLICT(chart_sha256, ln_policy, double_option, rule_mode) DO NOTHING",
        params![
            chart_sha256,
            record.ln_policy.as_str(),
            record.double_option.as_str(),
            rule_mode.as_str(),
            record.clear_type.as_str(),
            gauge_type_str(record.gauge_type),
            record.gauge_value,
            record.played_at,
            record.device_type.as_str(),
            clear_increment,
        ],
    )?;
    if inserted > 0 {
        return Ok(());
    }

    conn.execute(
        "UPDATE score_best
         SET play_count = play_count + 1,
             clear_count = clear_count + ?2
         WHERE chart_sha256 = ?1 AND ln_policy = ?3 AND double_option = ?4
           AND rule_mode = ?5",
        params![
            chart_sha256,
            clear_increment,
            record.ln_policy.as_str(),
            record.double_option.as_str(),
            rule_mode.as_str(),
        ],
    )?;

    let current_clear: String = conn.query_row(
        "SELECT clear_type
         FROM score_best
         WHERE chart_sha256 = ?1 AND ln_policy = ?2 AND double_option = ?3
           AND rule_mode = ?4",
        params![
            chart_sha256,
            record.ln_policy.as_str(),
            record.double_option.as_str(),
            rule_mode.as_str(),
        ],
        |row| row.get(0),
    )?;
    if record.clear_type as u8 > clear_rank_from_name(&current_clear) {
        conn.execute(
            "UPDATE score_best
             SET clear_type = ?2,
                 gauge_type = ?3,
                 gauge_value = ?4
             WHERE chart_sha256 = ?1 AND ln_policy = ?5 AND double_option = ?6
               AND rule_mode = ?7",
            params![
                chart_sha256,
                record.clear_type.as_str(),
                gauge_type_str(record.gauge_type),
                record.gauge_value,
                record.ln_policy.as_str(),
                record.double_option.as_str(),
                rule_mode.as_str(),
            ],
        )?;
    }
    Ok(())
}

pub(super) fn gauge_type_str(gauge_type: Option<GaugeType>) -> &'static str {
    gauge_type.map(GaugeType::as_str).unwrap_or("")
}

pub(super) fn is_counted_clear(clear_type: ClearType) -> bool {
    !matches!(clear_type, ClearType::NoPlay | ClearType::Failed)
}

pub(super) fn score_best_should_update_score(record: &ScoreRecord, current: ScoreBestRank) -> bool {
    let next = ScoreBestRank {
        ex_score: record.score.ex_score(),
        clear_rank: record.clear_type as u8,
        bp: score_record_bp(record),
        cb: score_record_cb(record),
        max_combo: record.score.max_combo,
    };
    (next.ex_score, std::cmp::Reverse(next.bp), std::cmp::Reverse(next.cb), next.max_combo)
        > (
            current.ex_score,
            std::cmp::Reverse(current.bp),
            std::cmp::Reverse(current.cb),
            current.max_combo,
        )
}

pub(super) fn score_best_should_update_clear(record: &ScoreRecord, current: ScoreBestRank) -> bool {
    record.clear_type as u8 > current.clear_rank
}

pub(super) fn score_record_bp(record: &ScoreRecord) -> u32 {
    if record.count_unprocessed_notes {
        record.score.bp_with_unprocessed_notes(record.total_notes)
    } else {
        record.score.bp()
    }
}

pub(super) fn score_record_cb(record: &ScoreRecord) -> u32 {
    if record.count_unprocessed_notes {
        record.score.cb_with_unprocessed_notes(record.total_notes)
    } else {
        record.score.cb()
    }
}

pub(super) fn clear_rank_from_name(value: &str) -> u8 {
    match value {
        "NoPlay" => ClearType::NoPlay as u8,
        "Failed" => ClearType::Failed as u8,
        "AssistEasy" => ClearType::AssistEasy as u8,
        "LightAssistEasy" => ClearType::LightAssistEasy as u8,
        "Easy" => ClearType::Easy as u8,
        "Normal" => ClearType::Normal as u8,
        "Hard" => ClearType::Hard as u8,
        "ExHard" => ClearType::ExHard as u8,
        "FullCombo" => ClearType::FullCombo as u8,
        "Perfect" => ClearType::Perfect as u8,
        "Max" => ClearType::Max as u8,
        _ => ClearType::NoPlay as u8,
    }
}
