use std::path::{Path, PathBuf};

use codex_atlas_rust_lib::domain::{
    format_usage, merge_layers, merge_layers_with_stats, AIData, Ability, AbilityKind, Stats,
    UserData,
};
use codex_atlas_rust_lib::store::{
    load_store, resolve_project_data_path, save_store, Store, StoreError,
};

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("codex-atlas-rust-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("data")).expect("create temp project data dir");

        Self { root }
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn raw_skill() -> Ability {
    Ability {
        id: "skill:atlas".to_string(),
        name: "Atlas Skill".to_string(),
        kind: AbilityKind::Skill,
        path: Some(PathBuf::from("skills/atlas/SKILL.md")),
        summary: "raw summary".to_string(),
        raw_tags: vec!["raw".to_string()],
        ai: AIData::default(),
        user: UserData::default(),
        stats: Stats::default(),
    }
}

#[test]
fn user_data_wins_over_ai_and_raw_layers() {
    let raw = raw_skill();
    let ai = AIData {
        summary_zh: Some("AI 摘要".to_string()),
        tags: vec!["ai".to_string()],
        call_template: Some("AI 模板".to_string()),
        scenarios: vec!["AI 场景".to_string()],
    };
    let user = UserData {
        alias: Some("用户别名".to_string()),
        tags: vec!["user".to_string()],
        note: Some("用户笔记".to_string()),
        favorite: true,
        hidden: true,
        custom_template: Some("用户模板".to_string()),
        ..UserData::default()
    };

    let ability = merge_layers(raw, ai, user);

    assert_eq!(ability.display_tags(), vec!["user"]);
    assert_eq!(ability.user.note.as_deref(), Some("用户笔记"));
    assert!(ability.user.favorite);
    assert!(ability.user.hidden);
    assert_eq!(ability.effective_call_template(), Some("用户模板"));
    assert_eq!(ability.ai.call_template.as_deref(), Some("AI 模板"));
}

#[test]
fn user_can_clear_tags_without_falling_back_to_ai_or_raw() {
    let raw = raw_skill();
    let ai = AIData {
        tags: vec!["ai".to_string()],
        ..AIData::default()
    };
    let user = UserData {
        tags: Vec::new(),
        tags_overridden: true,
        ..UserData::default()
    };

    let ability = merge_layers(raw, ai, user);

    assert!(ability.display_tags().is_empty());
}

#[test]
fn unknown_usage_is_not_rendered_as_zero() {
    let stats = Stats {
        usage_count: None,
        last_used_at: None,
    };

    let json = serde_json::to_string(&stats).expect("stats should serialize");

    assert!(json.contains("\"usage_count\":null"));
    assert_eq!(format_usage(stats.usage_count), "未统计");
}

#[test]
fn malformed_json_returns_default_store_without_panic() {
    let project = TestProject::new("bad-json-test");
    std::fs::write(project.root.join("data").join("store.json"), "{ bad json")
        .expect("write bad json");

    let result =
        load_store(&project.root, "store.json").expect("bad json should degrade to default store");

    assert!(result.store.abilities.is_empty());
    assert!(result.store.user_data.is_empty());
    assert!(result.store.ai_data.is_empty());
    assert!(result.store.stats.is_empty());
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn store_paths_stay_inside_project_data_dir() {
    let project = TestProject::new("path-boundary");

    let allowed = resolve_project_data_path(&project.root, Path::new("nested").join("store.json"))
        .expect("nested data file allowed");
    assert!(allowed.ends_with(Path::new("data").join("nested").join("store.json")));

    assert!(
        resolve_project_data_path(&project.root, Path::new("..").join("outside.json")).is_err()
    );
    assert!(
        resolve_project_data_path(&project.root, project.root.join("data").join("store.json"))
            .is_err()
    );

    let missing_project = project.root.join("missing-project");
    assert!(resolve_project_data_path(&missing_project, "store.json").is_err());
}

#[test]
fn old_schema_json_loads_with_defaults_without_warning() {
    let project = TestProject::new("old-schema");
    let json = r#"{
        "abilities": [{
            "id": "skill:legacy",
            "name": "Legacy Skill",
            "kind": "Skill",
            "summary": "legacy raw summary",
            "raw_tags": ["raw"],
            "ai": {
                "tags": ["ai"],
                "call_template": "AI legacy 模板"
            },
            "user": {
                "tags": ["user"],
                "note": "旧用户笔记"
            },
            "stats": {
                "usage_count": 9,
                "last_used_at": "2026-07-05T00:00:00Z"
            }
        }]
    }"#;
    std::fs::write(project.root.join("data").join("store.json"), json).expect("write legacy json");

    let result = load_store(&project.root, "store.json").expect("legacy json should load");

    assert!(result.warnings.is_empty());
    assert_eq!(result.store.abilities.len(), 1);
    assert_eq!(result.store.user_data["skill:legacy"].tags, vec!["user"]);
    assert_eq!(
        result.store.user_data["skill:legacy"].note.as_deref(),
        Some("旧用户笔记")
    );
    assert_eq!(
        result.store.ai_data["skill:legacy"]
            .call_template
            .as_deref(),
        Some("AI legacy 模板")
    );
    assert_eq!(result.store.stats["skill:legacy"].usage_count, Some(9));
    assert_eq!(result.store.abilities[0].user, UserData::default());
    assert_eq!(result.store.abilities[0].ai, AIData::default());
    assert_eq!(result.store.abilities[0].stats, Stats::default());
    assert_eq!(
        result.store.merged_abilities()[0].display_tags(),
        vec!["user"]
    );
}

#[test]
fn store_maps_are_canonical_overlay_for_merged_abilities() {
    let mut raw = raw_skill();
    raw.ai = AIData {
        tags: vec!["stale-ai".to_string()],
        ..AIData::default()
    };
    raw.user = UserData {
        tags: vec!["stale-user".to_string()],
        tags_overridden: true,
        ..UserData::default()
    };
    raw.stats = Stats {
        usage_count: Some(1),
        last_used_at: Some("旧统计".to_string()),
    };

    let mut store = Store {
        abilities: vec![raw],
        ..Store::default()
    };
    store.ai_data.insert(
        "skill:atlas".to_string(),
        AIData {
            tags: vec!["ai-map".to_string()],
            call_template: Some("AI map 模板".to_string()),
            ..AIData::default()
        },
    );
    store.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            tags: vec!["user-map".to_string()],
            tags_overridden: true,
            note: Some("map 用户笔记".to_string()),
            ..UserData::default()
        },
    );
    store.stats.insert(
        "skill:atlas".to_string(),
        Stats {
            usage_count: Some(42),
            last_used_at: Some("2026-07-05T00:00:00Z".to_string()),
        },
    );

    let merged = store.merged_abilities();

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].display_tags(), vec!["user-map"]);
    assert_eq!(merged[0].user.note.as_deref(), Some("map 用户笔记"));
    assert_eq!(merged[0].ai.call_template.as_deref(), Some("AI map 模板"));
    assert_eq!(merged[0].stats.usage_count, Some(42));
    assert_eq!(
        merged[0].stats.last_used_at.as_deref(),
        Some("2026-07-05T00:00:00Z")
    );
}

#[test]
fn merge_layers_with_stats_applies_stats() {
    let stats = Stats {
        usage_count: Some(7),
        last_used_at: Some("2026-07-05T00:00:00Z".to_string()),
    };

    let ability = merge_layers_with_stats(
        raw_skill(),
        AIData::default(),
        UserData::default(),
        stats.clone(),
    );

    assert_eq!(ability.stats, stats);
}

#[test]
fn save_store_creates_directories_and_roundtrips_overlay_maps() {
    let project = TestProject::new("roundtrip");
    let mut store = Store {
        abilities: vec![raw_skill()],
        ..Store::default()
    };
    store.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            favorite: true,
            tags: vec!["user".to_string()],
            ..UserData::default()
        },
    );
    store.ai_data.insert(
        "skill:atlas".to_string(),
        AIData {
            summary_zh: Some("中文摘要".to_string()),
            ..AIData::default()
        },
    );
    store.stats.insert(
        "skill:atlas".to_string(),
        Stats {
            usage_count: Some(3),
            last_used_at: None,
        },
    );

    save_store(
        &project.root,
        Path::new("nested").join("store.json"),
        &store,
    )
    .expect("save store");
    let result = load_store(&project.root, Path::new("nested").join("store.json"))
        .expect("load saved store");

    assert!(project
        .root
        .join("data")
        .join("nested")
        .join("store.json")
        .exists());
    assert!(result.warnings.is_empty());
    assert!(result.store.user_data["skill:atlas"].favorite);
    assert_eq!(
        result.store.ai_data["skill:atlas"].summary_zh.as_deref(),
        Some("中文摘要")
    );
    assert_eq!(result.store.stats["skill:atlas"].usage_count, Some(3));
}

#[test]
fn save_store_overwrites_existing_file_with_latest_overlay_maps() {
    let project = TestProject::new("overwrite-roundtrip");
    let mut original = Store {
        abilities: vec![raw_skill()],
        ..Store::default()
    };
    original.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            tags: vec!["old".to_string()],
            ..UserData::default()
        },
    );
    save_store(&project.root, "store.json", &original).expect("save original store");

    let mut updated = Store {
        abilities: vec![raw_skill()],
        ..Store::default()
    };
    updated.user_data.insert(
        "skill:atlas".to_string(),
        UserData {
            tags: vec!["new".to_string()],
            favorite: true,
            ..UserData::default()
        },
    );
    updated.stats.insert(
        "skill:atlas".to_string(),
        Stats {
            usage_count: Some(99),
            last_used_at: None,
        },
    );

    save_store(&project.root, "store.json", &updated).expect("overwrite existing store");
    let result = load_store(&project.root, "store.json").expect("load overwritten store");

    assert_eq!(result.store.user_data["skill:atlas"].tags, vec!["new"]);
    assert!(result.store.user_data["skill:atlas"].favorite);
    assert_eq!(result.store.stats["skill:atlas"].usage_count, Some(99));
    assert!(std::fs::read_dir(project.root.join("data"))
        .expect("read data dir")
        .all(|entry| !entry
            .expect("dir entry")
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
}

#[test]
fn save_store_refuses_to_replace_directory_target() {
    let project = TestProject::new("directory-target");
    let store = Store {
        abilities: vec![raw_skill()],
        ..Store::default()
    };
    let target_dir = project.root.join("data").join("store.json");
    std::fs::create_dir(&target_dir).expect("create directory at target path");

    let result = save_store(&project.root, "store.json", &store);

    match result {
        Err(StoreError::Io(error)) => assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput),
        other => panic!("expected InvalidInput IO error, got {other:?}"),
    }
    assert!(target_dir.is_dir());
}

#[cfg(any(unix, windows))]
#[test]
fn store_path_rejects_data_symlink_outside_project_when_supported() {
    let project = TestProject::new("data-symlink");
    let outside = std::env::temp_dir().join(format!(
        "codex-atlas-rust-outside-data-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir_all(&outside).expect("create outside target");
    std::fs::remove_dir(project.root.join("data")).expect("remove real data dir");

    if create_dir_symlink(&outside, &project.root.join("data")).is_err() {
        let _ = std::fs::remove_dir_all(&outside);
        return;
    }

    let result = resolve_project_data_path(&project.root, "store.json");

    let _ = remove_dir_symlink(&project.root.join("data"));
    let _ = std::fs::remove_dir_all(&outside);
    assert!(result.is_err());
}

#[cfg(unix)]
fn create_dir_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(unix)]
fn remove_dir_symlink(link: &Path) -> std::io::Result<()> {
    std::fs::remove_file(link)
}

#[cfg(windows)]
fn create_dir_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(windows)]
fn remove_dir_symlink(link: &Path) -> std::io::Result<()> {
    std::fs::remove_dir(link)
}
