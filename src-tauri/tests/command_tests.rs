#[allow(dead_code)]
mod fixtures;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use codex_atlas_rust_lib::{
    app_state::{
        default_scan_roots_for_home, default_stats_roots_for_home, AppState, SharedAppState,
    },
    commands::{
        copy_call_template_state, get_ability_state, list_abilities_state, refresh_scan_state,
        refresh_stats_state, update_user_data_state, UserDataPatch,
    },
    domain::{AIData, Ability, AbilityKind, Stats, UserData},
    scanner::ScanRoots,
    stats::StatsRoots,
    store::{load_stats_cache, load_store, save_store, Store},
};
use fixtures::ScannerFixture;
use serde_json::json;

fn raw_skill(id: &str, name: &str, summary: &str) -> Ability {
    Ability {
        id: id.to_string(),
        name: name.to_string(),
        kind: AbilityKind::Skill,
        path: Some(PathBuf::from(format!(
            "skills/{}/SKILL.md",
            id.trim_start_matches("skill:")
        ))),
        summary: summary.to_string(),
        raw_tags: vec!["raw".to_string()],
        ..Ability::default()
    }
}

fn shared_state(
    project_dir: PathBuf,
    scan_roots: ScanRoots,
    stats_roots: StatsRoots,
) -> SharedAppState {
    Arc::new(RwLock::new(AppState::new(
        project_dir,
        scan_roots,
        stats_roots,
    )))
}

fn usage_count(state: &SharedAppState, ability_id: &str) -> Option<u64> {
    list_abilities_state(state)
        .expect("list abilities")
        .into_iter()
        .find(|ability| ability.id == ability_id)
        .and_then(|ability| ability.stats.usage_count)
}

fn nullable_user_patch_state(name: &str) -> (ScannerFixture, SharedAppState) {
    let fixture = ScannerFixture::new(name);
    let mut store = Store {
        abilities: vec![raw_skill("skill:atlas", "Atlas Skill", "raw summary")],
        ..Store::default()
    };
    store.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            alias: Some("旧别名".to_string()),
            note: Some("旧笔记".to_string()),
            custom_template: Some("用户旧模板".to_string()),
            ..UserData::default()
        },
    );
    store.ai_data.insert(
        "skill:atlas".to_string(),
        AIData {
            call_template: Some("AI 模板".to_string()),
            ..AIData::default()
        },
    );
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots::default(),
    );

    (fixture, state)
}

fn seed_store_with_old_atlas_stats(fixture: &ScannerFixture) {
    let mut store = Store {
        abilities: vec![raw_skill("skill:atlas", "Atlas Skill", "raw summary")],
        ..Store::default()
    };
    store
        .stats
        .insert("skill:atlas".to_string(), Stats::counted(9));
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
}

#[test]
fn default_roots_use_real_codex_locations_under_home() {
    let fixture = ScannerFixture::new("commands-default-roots");
    fixture.write_text(".codex/sessions/2026/one.jsonl", "");
    fixture.write_text(".codex/sessions/2026/two.jsonl", "");
    fixture.write_text(".codex/plugins/plugins.json", "{\"plugins\":[]}");
    fixture.write_text(".codex/tools.json", "{\"tools\":[]}");
    fixture.write_text(".codex/apps/apps.json", "{\"apps\":[]}");

    let scan_roots = default_scan_roots_for_home(&fixture.path("."));
    let stats_roots = default_stats_roots_for_home(&fixture.path("."));

    assert!(scan_roots
        .skill_roots
        .contains(&fixture.path(".codex/skills")));
    assert!(scan_roots
        .skill_roots
        .contains(&fixture.path(".agents/skills")));
    assert!(scan_roots
        .plugins_roots
        .contains(&fixture.path(".codex/plugins")));
    assert_eq!(
        scan_roots.plugins_manifests,
        vec![fixture.path(".codex/plugins/plugins.json")]
    );
    assert_eq!(
        scan_roots.tools_manifests,
        vec![fixture.path(".codex/tools.json")]
    );
    assert_eq!(
        scan_roots.apps_manifests,
        vec![fixture.path(".codex/apps/apps.json")]
    );
    assert_eq!(stats_roots.conversation_files.len(), 2);
    assert!(stats_roots
        .conversation_files
        .iter()
        .all(|path| path.ends_with("one.jsonl") || path.ends_with("two.jsonl")));
}

#[test]
fn refresh_scan_with_default_roots_includes_catalog_manifests() {
    let fixture = ScannerFixture::new("commands-default-catalog-refresh");
    fixture.write_text(
        ".codex/plugins/plugins.json",
        r#"{
  "plugins": [
    { "name": "superpowers", "description": "Workflow plugin" }
  ]
}"#,
    );
    fixture.write_text(
        ".codex/tools/tools.json",
        r#"{
  "tools": [
    { "name": "shell", "description": "Run shell commands" }
  ]
}"#,
    );
    fixture.write_text(
        ".codex/apps/apps.json",
        r#"{
  "apps": [
    { "name": "github", "description": "GitHub app" }
  ]
}"#,
    );
    let state = shared_state(
        fixture.path("."),
        default_scan_roots_for_home(&fixture.path(".")),
        StatsRoots::default(),
    );

    let summary = refresh_scan_state(&state).expect("refresh scan");
    let abilities = list_abilities_state(&state).expect("list refreshed abilities");

    assert_eq!(summary.ability_count, 3);
    assert!(summary.warnings.is_empty());
    assert_eq!(
        abilities
            .iter()
            .map(|ability| (&ability.id, &ability.kind, &ability.summary))
            .collect::<Vec<_>>(),
        vec![
            (
                &"plugin:superpowers".to_string(),
                &AbilityKind::Plugin,
                &"Workflow plugin".to_string()
            ),
            (
                &"tool:shell".to_string(),
                &AbilityKind::Tool,
                &"Run shell commands".to_string()
            ),
            (
                &"app:github".to_string(),
                &AbilityKind::App,
                &"GitHub app".to_string()
            ),
        ]
    );
}

#[test]
fn refresh_stats_does_not_save_when_default_discovery_hits_file_limit() {
    let fixture = ScannerFixture::new("commands-stats-discovery-file-limit");
    seed_store_with_old_atlas_stats(&fixture);
    for index in 0..513 {
        fixture.write_text(
            format!(".codex/sessions/2026/{index:03}.jsonl"),
            "{\"role\":\"user\",\"content\":\"没有点名\"}\n",
        );
    }
    let stats_roots = default_stats_roots_for_home(&fixture.path("."));
    let state = shared_state(fixture.path("."), ScanRoots::default(), stats_roots);

    let summary = refresh_stats_state(&state).expect("discovery truncation should be no-op");

    assert!(!summary.complete);
    assert!(!summary.saved_cache);
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.contains("数量上限")));
    assert_eq!(usage_count(&state, "skill:atlas"), Some(9));
    let saved = load_store(fixture.path("."), "store.json").expect("load store");
    assert_eq!(saved.store.stats["skill:atlas"].usage_count, Some(9));
    assert!(load_stats_cache(fixture.path("."))
        .expect("load missing stats cache")
        .is_empty());
}

#[test]
fn refresh_stats_does_not_save_when_default_discovery_root_is_not_directory() {
    let fixture = ScannerFixture::new("commands-stats-root-not-directory");
    seed_store_with_old_atlas_stats(&fixture);
    fixture.write_text(".codex/sessions", "not a directory");
    let stats_roots = default_stats_roots_for_home(&fixture.path("."));
    let state = shared_state(fixture.path("."), ScanRoots::default(), stats_roots);

    let summary = refresh_stats_state(&state).expect("bad discovery root should be no-op");

    assert!(!summary.complete);
    assert!(!summary.saved_cache);
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.contains("非真实目录")));
    assert_eq!(usage_count(&state, "skill:atlas"), Some(9));
    let saved = load_store(fixture.path("."), "store.json").expect("load store");
    assert_eq!(saved.store.stats["skill:atlas"].usage_count, Some(9));
}

#[cfg(any(unix, windows))]
#[test]
fn refresh_stats_does_not_save_when_default_discovery_skips_child_symlink() {
    let fixture = ScannerFixture::new("commands-stats-child-symlink");
    seed_store_with_old_atlas_stats(&fixture);
    fixture.write_text(
        ".codex/sessions/readable.jsonl",
        "{\"role\":\"user\",\"content\":\"没有点名\"}\n",
    );
    fixture.write_text(
        "outside/external.jsonl",
        "{\"role\":\"user\",\"content\":\"$atlas\"}\n",
    );
    if create_file_symlink(
        &fixture.path("outside/external.jsonl"),
        &fixture.path(".codex/sessions/linked.jsonl"),
    )
    .is_err()
    {
        return;
    }
    let stats_roots = default_stats_roots_for_home(&fixture.path("."));
    let state = shared_state(fixture.path("."), ScanRoots::default(), stats_roots);

    let summary = refresh_stats_state(&state).expect("symlink discovery should be no-op");

    assert!(!summary.complete);
    assert!(!summary.saved_cache);
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.contains("symlink")));
    assert_eq!(usage_count(&state, "skill:atlas"), Some(9));
    let saved = load_store(fixture.path("."), "store.json").expect("load store");
    assert_eq!(saved.store.stats["skill:atlas"].usage_count, Some(9));
}

#[test]
fn user_data_patch_json_null_clears_nullable_fields() {
    let (_fixture, state) = nullable_user_patch_state("commands-json-null-clear");
    let patch: UserDataPatch = serde_json::from_value(json!({
        "alias": null,
        "note": null,
        "custom_template": null
    }))
    .expect("deserialize null patch");

    assert_eq!(patch.alias, Some(None));
    assert_eq!(patch.note, Some(None));
    assert_eq!(patch.custom_template, Some(None));

    let ability =
        update_user_data_state(&state, "skill:atlas", patch).expect("clear nullable fields");

    assert_eq!(ability.user.alias, None);
    assert_eq!(ability.user.note, None);
    assert_eq!(ability.user.custom_template, None);
    assert_eq!(ability.ai.call_template.as_deref(), Some("AI 模板"));
    assert_eq!(ability.effective_call_template(), Some("AI 模板"));
}

#[test]
fn user_data_patch_json_missing_nullable_fields_preserves_existing() {
    let (_fixture, state) = nullable_user_patch_state("commands-json-missing-preserve");
    let patch: UserDataPatch = serde_json::from_value(json!({})).expect("deserialize empty patch");

    assert_eq!(patch.alias, None);
    assert_eq!(patch.note, None);
    assert_eq!(patch.custom_template, None);

    let ability = update_user_data_state(&state, "skill:atlas", patch).expect("apply empty patch");

    assert_eq!(ability.user.alias.as_deref(), Some("旧别名"));
    assert_eq!(ability.user.note.as_deref(), Some("旧笔记"));
    assert_eq!(ability.user.custom_template.as_deref(), Some("用户旧模板"));
}

#[test]
fn user_data_patch_json_strings_update_nullable_fields() {
    let (_fixture, state) = nullable_user_patch_state("commands-json-strings-update");
    let patch: UserDataPatch = serde_json::from_value(json!({
        "alias": "新别名",
        "note": "新笔记",
        "custom_template": "新模板"
    }))
    .expect("deserialize string patch");

    assert_eq!(patch.alias, Some(Some("新别名".to_string())));
    assert_eq!(patch.note, Some(Some("新笔记".to_string())));
    assert_eq!(patch.custom_template, Some(Some("新模板".to_string())));

    let ability =
        update_user_data_state(&state, "skill:atlas", patch).expect("update nullable fields");

    assert_eq!(ability.user.alias.as_deref(), Some("新别名"));
    assert_eq!(ability.user.note.as_deref(), Some("新笔记"));
    assert_eq!(ability.user.custom_template.as_deref(), Some("新模板"));
}

#[test]
fn list_abilities_returns_merged_data() {
    let fixture = ScannerFixture::new("commands-list-merged");
    let mut stale_raw = raw_skill("skill:atlas", "Atlas Skill", "raw summary");
    stale_raw.ai = AIData {
        tags: vec!["stale-ai".to_string()],
        ..AIData::default()
    };
    stale_raw.user = UserData {
        tags: vec!["stale-user".to_string()],
        tags_overridden: true,
        ..UserData::default()
    };
    stale_raw.stats = Stats::counted(1);

    let mut store = Store {
        abilities: vec![stale_raw],
        ..Store::default()
    };
    store.ai_data.insert(
        "skill:atlas".to_string(),
        AIData {
            tags: vec!["ai-map".to_string()],
            call_template: Some("AI 模板".to_string()),
            ..AIData::default()
        },
    );
    store.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            tags: vec!["user-map".to_string()],
            tags_overridden: true,
            note: Some("用户笔记".to_string()),
            ..UserData::default()
        },
    );
    store
        .stats
        .insert("skill:atlas".to_string(), Stats::counted(42));
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots::default(),
    );

    let abilities = list_abilities_state(&state).expect("list abilities");

    assert_eq!(abilities.len(), 1);
    assert_eq!(abilities[0].display_tags(), vec!["user-map"]);
    assert_eq!(abilities[0].user.note.as_deref(), Some("用户笔记"));
    assert_eq!(abilities[0].ai.call_template.as_deref(), Some("AI 模板"));
    assert_eq!(abilities[0].stats.usage_count, Some(42));
}

#[test]
fn update_user_data_persists_without_overwriting_ai_data() {
    let fixture = ScannerFixture::new("commands-update-user");
    let mut store = Store {
        abilities: vec![raw_skill("skill:atlas", "Atlas Skill", "raw summary")],
        ..Store::default()
    };
    store.ai_data.insert(
        "skill:atlas".to_string(),
        AIData {
            call_template: Some("AI 模板".to_string()),
            tags: vec!["ai".to_string()],
            ..AIData::default()
        },
    );
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots::default(),
    );

    let ability = update_user_data_state(
        &state,
        "skill:atlas",
        UserDataPatch {
            note: Some(Some("保留我的说明".to_string())),
            favorite: Some(true),
            custom_template: Some(Some("用户模板".to_string())),
            ..UserDataPatch::default()
        },
    )
    .expect("update user data");

    assert_eq!(ability.user.note.as_deref(), Some("保留我的说明"));
    assert!(ability.user.favorite);
    assert_eq!(ability.effective_call_template(), Some("用户模板"));
    assert_eq!(ability.ai.call_template.as_deref(), Some("AI 模板"));

    let saved = load_store(fixture.path("."), "store.json").expect("load saved store");
    assert_eq!(
        saved.store.user_data["skill:atlas"].note.as_deref(),
        Some("保留我的说明")
    );
    assert!(saved.store.user_data["skill:atlas"].favorite);
    assert_eq!(
        saved.store.ai_data["skill:atlas"].call_template.as_deref(),
        Some("AI 模板")
    );
    assert_eq!(saved.store.ai_data["skill:atlas"].tags, vec!["ai"]);
}

#[test]
fn update_user_data_can_clear_user_fields_without_overwriting_ai_data() {
    let fixture = ScannerFixture::new("commands-update-clear-user");
    let mut store = Store {
        abilities: vec![raw_skill("skill:atlas", "Atlas Skill", "raw summary")],
        ..Store::default()
    };
    store.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            tags: vec!["old".to_string()],
            note: Some("旧笔记".to_string()),
            custom_template: Some("用户旧模板".to_string()),
            ..UserData::default()
        },
    );
    store.ai_data.insert(
        "skill:atlas".to_string(),
        AIData {
            call_template: Some("AI 模板".to_string()),
            ..AIData::default()
        },
    );
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots::default(),
    );

    let ability = update_user_data_state(
        &state,
        "skill:atlas",
        UserDataPatch {
            tags: Some(Vec::new()),
            note: Some(None),
            custom_template: Some(None),
            ..UserDataPatch::default()
        },
    )
    .expect("clear user fields");

    assert!(ability.user.tags.is_empty());
    assert!(ability.user.tags_overridden);
    assert_eq!(ability.user.note, None);
    assert_eq!(ability.user.custom_template, None);
    assert_eq!(ability.ai.call_template.as_deref(), Some("AI 模板"));
    assert_eq!(ability.effective_call_template(), Some("AI 模板"));
}

#[test]
fn refresh_scan_keeps_existing_user_data() {
    let fixture = ScannerFixture::new("commands-refresh-scan");
    fixture.write_text(
        "skills/atlas/SKILL.md",
        r#"---
name: atlas
description: refreshed summary
---
"#,
    );
    let mut store = Store {
        abilities: vec![raw_skill("skill:old", "Old Skill", "old summary")],
        ..Store::default()
    };
    store.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            note: Some("扫描前的用户笔记".to_string()),
            favorite: true,
            ..UserData::default()
        },
    );
    store.ai_data.insert(
        "skill:atlas".to_string(),
        AIData {
            call_template: Some("AI atlas".to_string()),
            ..AIData::default()
        },
    );
    store
        .stats
        .insert("skill:atlas".to_string(), Stats::counted(7));
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::new().with_skill_root(fixture.path("skills")),
        StatsRoots::default(),
    );

    let summary = refresh_scan_state(&state).expect("refresh scan");
    let ability = get_ability_state(&state, "skill:atlas").expect("get refreshed ability");

    assert_eq!(summary.ability_count, 1);
    assert_eq!(ability.summary, "refreshed summary");
    assert_eq!(ability.user.note.as_deref(), Some("扫描前的用户笔记"));
    assert!(ability.user.favorite);
    assert_eq!(ability.ai.call_template.as_deref(), Some("AI atlas"));
    assert_eq!(ability.stats.usage_count, Some(7));

    let saved = load_store(fixture.path("."), "store.json").expect("load refreshed store");
    assert_eq!(saved.store.abilities.len(), 1);
    assert_eq!(saved.store.abilities[0].id, "skill:atlas");
    assert_eq!(
        saved.store.user_data["skill:atlas"].note.as_deref(),
        Some("扫描前的用户笔记")
    );
    assert_eq!(saved.store.stats["skill:atlas"].usage_count, Some(7));
}

#[test]
fn refresh_scan_with_empty_roots_is_noop_and_keeps_saved_abilities() {
    let fixture = ScannerFixture::new("commands-refresh-scan-empty");
    let store = Store {
        abilities: vec![raw_skill("skill:atlas", "Atlas Skill", "raw summary")],
        ..Store::default()
    };
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots::default(),
    );

    let summary = refresh_scan_state(&state).expect("empty scan roots should be a no-op");

    assert_eq!(summary.ability_count, 1);
    assert_eq!(summary.warning_count, 1);
    assert!(summary.warnings[0].contains("扫描根目录为空"));
    assert_eq!(
        list_abilities_state(&state).expect("list after no-op")[0].id,
        "skill:atlas"
    );
    let saved = load_store(fixture.path("."), "store.json").expect("load store");
    assert_eq!(saved.store.abilities.len(), 1);
    assert_eq!(saved.store.abilities[0].id, "skill:atlas");
}

#[test]
fn refresh_stats_preserves_unknown_for_unseen_abilities() {
    let fixture = ScannerFixture::new("commands-refresh-stats-incomplete");
    let mut store = Store {
        abilities: vec![
            raw_skill("skill:atlas", "Atlas Skill", "raw summary"),
            raw_skill("skill:unseen", "Unseen Skill", "raw summary"),
        ],
        ..Store::default()
    };
    store
        .stats
        .insert("skill:atlas".to_string(), Stats::counted(9));
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots {
            conversation_files: vec![fixture.path("conversations/missing.jsonl")],
            ..StatsRoots::default()
        },
    );

    let summary = refresh_stats_state(&state).expect("refresh stats");
    let abilities = list_abilities_state(&state).expect("list after incomplete stats");

    assert!(!summary.complete);
    assert!(!summary.saved_cache);
    assert_eq!(
        abilities
            .iter()
            .find(|ability| ability.id == "skill:atlas")
            .and_then(|ability| ability.stats.usage_count),
        Some(9)
    );
    assert_eq!(
        abilities
            .iter()
            .find(|ability| ability.id == "skill:unseen")
            .and_then(|ability| ability.stats.usage_count),
        None
    );
    assert!(load_stats_cache(fixture.path("."))
        .expect("load missing stats cache")
        .is_empty());
}

#[test]
fn refresh_stats_with_empty_roots_does_not_save_zero_counts() {
    let fixture = ScannerFixture::new("commands-refresh-stats-empty");
    let mut store = Store {
        abilities: vec![
            raw_skill("skill:atlas", "Atlas Skill", "raw summary"),
            raw_skill("skill:unseen", "Unseen Skill", "raw summary"),
        ],
        ..Store::default()
    };
    store
        .stats
        .insert("skill:atlas".to_string(), Stats::counted(9));
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots::default(),
    );

    let summary = refresh_stats_state(&state).expect("empty stats roots should be a no-op");

    assert!(!summary.complete);
    assert!(!summary.saved_cache);
    assert!(summary.warnings[0].contains("统计根目录为空"));
    assert_eq!(usage_count(&state, "skill:atlas"), Some(9));
    assert_eq!(usage_count(&state, "skill:unseen"), None);
    assert!(load_stats_cache(fixture.path("."))
        .expect("load missing stats cache")
        .is_empty());
}

#[test]
fn refresh_stats_saves_complete_counts_and_cache_for_scanned_abilities() {
    let fixture = ScannerFixture::new("commands-refresh-stats-complete");
    let store = Store {
        abilities: vec![
            raw_skill("skill:atlas", "Atlas Skill", "raw summary"),
            raw_skill("skill:unused", "Unused Skill", "raw summary"),
        ],
        ..Store::default()
    };
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    fixture.write_text(
        "conversations/session.jsonl",
        "{\"role\":\"user\",\"content\":\"请用 $atlas\"}\n",
    );
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots {
            conversation_files: vec![fixture.path("conversations/session.jsonl")],
            ..StatsRoots::default()
        },
    );

    let summary = refresh_stats_state(&state).expect("refresh stats");
    let abilities = list_abilities_state(&state).expect("list after complete stats");
    let cache = load_stats_cache(fixture.path(".")).expect("load saved stats cache");

    assert!(summary.complete);
    assert!(summary.saved_cache);
    assert_eq!(summary.ability_count, 2);
    assert_eq!(
        abilities
            .iter()
            .find(|ability| ability.id == "skill:atlas")
            .and_then(|ability| ability.stats.usage_count),
        Some(1)
    );
    assert_eq!(
        abilities
            .iter()
            .find(|ability| ability.id == "skill:unused")
            .and_then(|ability| ability.stats.usage_count),
        Some(0)
    );
    assert_eq!(cache["skill:atlas"].usage_count, Some(1));
    assert_eq!(cache["skill:unused"].usage_count, Some(0));
}

#[test]
fn refresh_stats_cache_save_failure_keeps_store_and_memory_unchanged() {
    let fixture = ScannerFixture::new("commands-refresh-stats-cache-failure");
    let mut store = Store {
        abilities: vec![raw_skill("skill:atlas", "Atlas Skill", "raw summary")],
        ..Store::default()
    };
    store
        .stats
        .insert("skill:atlas".to_string(), Stats::counted(9));
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    std::fs::create_dir_all(fixture.path("data/stats-cache.json"))
        .expect("make stats cache path unwritable as file");
    fixture.write_text(
        "conversations/session.jsonl",
        "{\"role\":\"user\",\"content\":\"请用 $atlas\"}\n",
    );
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots {
            conversation_files: vec![fixture.path("conversations/session.jsonl")],
            ..StatsRoots::default()
        },
    );

    let error = refresh_stats_state(&state).expect_err("cache save should fail");

    assert!(error.message.contains("存储目标不是文件"));
    assert_eq!(usage_count(&state, "skill:atlas"), Some(9));
    let saved = load_store(fixture.path("."), "store.json").expect("load store");
    assert_eq!(saved.store.stats["skill:atlas"].usage_count, Some(9));
}

#[test]
fn get_ability_returns_merged_data() {
    let fixture = ScannerFixture::new("commands-get-merged");
    let mut store = Store {
        abilities: vec![raw_skill("skill:atlas", "Atlas Skill", "raw summary")],
        ..Store::default()
    };
    store.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            alias: Some("地图册".to_string()),
            ..UserData::default()
        },
    );
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots::default(),
    );

    let ability = get_ability_state(&state, "skill:atlas").expect("get ability");

    assert_eq!(ability.user.alias.as_deref(), Some("地图册"));
}

#[test]
fn copy_call_template_prefers_user_then_ai_then_default() {
    let fixture = ScannerFixture::new("commands-copy-template");
    let mut store = Store {
        abilities: vec![
            raw_skill("skill:user-template", "User Template", "raw summary"),
            raw_skill("skill:ai-template", "AI Template", "raw summary"),
            raw_skill("skill:atlas", "Atlas Skill", "raw summary"),
            raw_skill(
                "superpowers:test-driven-development",
                "test-driven-development",
                "raw summary",
            ),
        ],
        ..Store::default()
    };
    store.user_data.insert(
        "skill:user-template".to_string(),
        UserData {
            custom_template: Some("用户模板".to_string()),
            ..UserData::default()
        },
    );
    store.ai_data.insert(
        "skill:ai-template".to_string(),
        AIData {
            call_template: Some("AI 模板".to_string()),
            ..AIData::default()
        },
    );
    save_store(fixture.path("."), "store.json", &store).expect("seed store");
    let state = shared_state(
        fixture.path("."),
        ScanRoots::default(),
        StatsRoots::default(),
    );

    assert_eq!(
        copy_call_template_state(&state, "skill:user-template").expect("user template"),
        "用户模板"
    );
    assert_eq!(
        copy_call_template_state(&state, "skill:ai-template").expect("ai template"),
        "AI 模板"
    );
    assert_eq!(
        copy_call_template_state(&state, "skill:atlas").expect("default skill template"),
        "$atlas"
    );
    assert_eq!(
        copy_call_template_state(&state, "superpowers:test-driven-development")
            .expect("default plugin skill template"),
        "$superpowers:test-driven-development"
    );
}

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}
