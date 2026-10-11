use super::*;
use crate::ir::types::{IrRivalEntry, IrRivalProfile};

fn profile_with_entries() -> ProfileConfig {
    ProfileConfig::new_default("test", "Test", 0)
}

fn ir_rival(id: &str, name: &str) -> IrRivalEntry {
    IrRivalEntry {
        player_id: id.to_string(),
        relation_type: "rival".to_string(),
        profile: Some(IrRivalProfile { display_name: name.to_string(), bio: None }),
    }
}

#[test]
fn ir_cli_provider_protocol_matches_builtin_aliases() {
    assert!(auth::same_provider_protocol("bmz", "bmz-official"));
    assert!(auth::same_provider_protocol("rianIR", "rian-ir"));
    assert!(!auth::same_provider_protocol("bmz", "rian-ir"));
    assert_eq!(auth::canonical_provider_protocol("bmz-official"), "bmz");
    assert_eq!(auth::canonical_provider_protocol("rianIR"), "rian-ir");
}

#[test]
fn sync_ir_rivals_adds_updates_and_prunes() {
    let mut profile = profile_with_entries();

    // 追加。
    assert!(sync_ir_rivals_into_profile(
        &mut profile,
        "bmz-official",
        &[ir_rival("p1", "Alice"), ir_rival("p2", "Bob")],
    ));
    assert_eq!(profile.rival.entries.len(), 2);
    profile.rival.active_rival = profile.rival.entries[1].id.clone();

    // 変化なしなら false。
    assert!(!sync_ir_rivals_into_profile(
        &mut profile,
        "bmz-official",
        &[ir_rival("p1", "Alice"), ir_rival("p2", "Bob")],
    ));

    // 表示名更新 + サーバーから消えたものは削除。
    assert!(
        sync_ir_rivals_into_profile(&mut profile, "bmz-official", &[ir_rival("p1", "Alice2")],)
    );
    assert_eq!(profile.rival.entries.len(), 1);
    assert_eq!(profile.rival.entries[0].display_name, "Alice2");
    assert_eq!(profile.rival.entries[0].ir_user_id, "p1");
    assert!(profile.rival.active_rival.is_empty());
}

#[test]
fn sync_ir_rivals_keeps_manual_entries() {
    use crate::config::profile_config::{RivalEntry, RivalSourceConfig};
    let mut profile = profile_with_entries();
    profile.rival.entries.push(RivalEntry {
        id: "local-1".to_string(),
        display_name: "LocalFriend".to_string(),
        source: RivalSourceConfig::LocalProfile,
        profile_id: "other".to_string(),
        path: String::new(),
        ir_service: String::new(),
        ir_user_id: String::new(),
    });

    assert!(!sync_ir_rivals_into_profile(&mut profile, "bmz-official", &[]));
    assert_eq!(profile.rival.entries.len(), 1);
    assert_eq!(profile.rival.entries[0].id, "local-1");
}
