#[allow(dead_code)]
mod fixtures;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use codex_atlas_rust_lib::{
    app_state::{AppState, SharedAppState},
    commands::{list_skill_files_state, open_skill_detail_window_state, read_skill_file_state},
    domain::{Ability, AbilityKind},
    scanner::ScanRoots,
    skilldoc::skill_detail_window_target,
    stats::StatsRoots,
    store::{save_store, Store},
};
use fixtures::ScannerFixture;
use serde_json::Value;

fn skill_ability(id: &str, name: &str, skill_md_path: PathBuf) -> Ability {
    Ability {
        id: id.to_string(),
        name: name.to_string(),
        kind: AbilityKind::Skill,
        path: Some(skill_md_path),
        summary: "Skill detail fixture".to_string(),
        ..Ability::default()
    }
}

fn non_skill_ability(id: &str, name: &str) -> Ability {
    Ability {
        id: id.to_string(),
        name: name.to_string(),
        kind: AbilityKind::Tool,
        path: None,
        summary: "Not a skill".to_string(),
        ..Ability::default()
    }
}

fn shared_state(project_dir: PathBuf, abilities: Vec<Ability>) -> SharedAppState {
    save_store(
        &project_dir,
        "store.json",
        &Store {
            abilities,
            ..Store::default()
        },
    )
    .expect("seed store");

    Arc::new(RwLock::new(AppState::new(
        project_dir,
        ScanRoots::default(),
        StatsRoots::default(),
    )))
}

fn listed_paths(state: &SharedAppState, id: &str) -> Vec<String> {
    list_skill_files_state(state, id)
        .expect("list skill files")
        .files
        .into_iter()
        .map(|entry| entry.relative_path)
        .collect()
}

#[test]
fn lists_only_readable_files_inside_skill_root() {
    let fixture = ScannerFixture::new("skilldoc-list-readable");
    fixture.write_text("skills/atlas/SKILL.md", "# Atlas\n\nReadable skill.");
    fixture.write_text("skills/atlas/README.md", "Extra notes.");
    fixture.write_text("skills/atlas/references/guide.md", "Guide.");
    fixture.write_text(
        "skills/atlas/assets/logo.png",
        "not a real png but still skipped by extension",
    );
    let state = shared_state(
        fixture.path("."),
        vec![skill_ability(
            "skill:atlas",
            "atlas",
            fixture.path("skills/atlas/SKILL.md"),
        )],
    );

    let detail = open_skill_detail_window_state(&state, "skill:atlas").expect("validate detail");
    let paths = listed_paths(&state, "skill:atlas");

    assert_eq!(detail.skill_id, "skill:atlas");
    assert!(detail.root_path.ends_with("skills/atlas"));
    assert_eq!(
        paths,
        vec![
            "README.md".to_string(),
            "SKILL.md".to_string(),
            "references/guide.md".to_string(),
        ]
    );
}

#[test]
fn rejects_relative_path_escape() {
    let fixture = ScannerFixture::new("skilldoc-reject-escape");
    fixture.write_text("skills/atlas/SKILL.md", "# Atlas");
    fixture.write_text("outside/secret.md", "secret");
    let state = shared_state(
        fixture.path("."),
        vec![skill_ability(
            "skill:atlas",
            "atlas",
            fixture.path("skills/atlas/SKILL.md"),
        )],
    );

    let parent_escape = read_skill_file_state(&state, "skill:atlas", "../outside/secret.md")
        .expect_err("parent traversal must be rejected");
    let absolute_escape = read_skill_file_state(
        &state,
        "skill:atlas",
        fixture.path("outside/secret.md").to_string_lossy().as_ref(),
    )
    .expect_err("absolute path must be rejected");

    assert!(parent_escape.message.contains("相对路径"));
    assert!(absolute_escape.message.contains("绝对路径"));

    if create_file_symlink(
        &fixture.path("outside/secret.md"),
        &fixture.path("skills/atlas/linked-secret.md"),
    )
    .is_ok()
    {
        let symlink_escape = read_skill_file_state(&state, "skill:atlas", "linked-secret.md")
            .expect_err("symlink target outside root must be rejected");
        assert!(symlink_escape.message.contains("越过 Skill 目录"));
    }
}

#[test]
fn skips_dependency_build_binary_and_oversized_files() {
    let fixture = ScannerFixture::new("skilldoc-skip-policy");
    fixture.write_text("skills/atlas/SKILL.md", "# Atlas");
    fixture.write_text("skills/atlas/src/main.rs", "fn main() {}");
    fixture.write_text("skills/atlas/node_modules/pkg/README.md", "dependency");
    fixture.write_text("skills/atlas/target/debug/out.txt", "build");
    fixture.write_text("skills/atlas/.git/config", "git");
    fixture.write_text("skills/atlas/.cache/cache.txt", "cache");
    fixture.write_bytes("skills/atlas/assets/icon.png", &[0x89, b'P', b'N', b'G']);
    fixture.write_text("skills/atlas/huge.txt", &"x".repeat(512 * 1024 + 1));
    let state = shared_state(
        fixture.path("."),
        vec![skill_ability(
            "skill:atlas",
            "atlas",
            fixture.path("skills/atlas/SKILL.md"),
        )],
    );

    let result = list_skill_files_state(&state, "skill:atlas").expect("list skill files");
    let paths = result
        .files
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect::<Vec<_>>();

    assert_eq!(paths, vec!["SKILL.md", "src/main.rs"]);
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.contains("超过最大文件大小")));
}

#[test]
fn binary_detection_only_samples_the_file_prefix() {
    let fixture = ScannerFixture::new("skilldoc-binary-prefix-sample");
    fixture.write_text("skills/atlas/SKILL.md", "# Atlas");
    let mut late_invalid_utf8 = vec![b'a'; 8193];
    late_invalid_utf8[8192] = 0xFF;
    fixture.write_bytes("skills/atlas/late-invalid.txt", &late_invalid_utf8);
    let state = shared_state(
        fixture.path("."),
        vec![skill_ability(
            "skill:atlas",
            "atlas",
            fixture.path("skills/atlas/SKILL.md"),
        )],
    );

    let paths = listed_paths(&state, "skill:atlas");

    assert!(paths.contains(&"late-invalid.txt".to_string()));
}

#[test]
fn read_file_returns_markdown_text_code_or_config_content() {
    let fixture = ScannerFixture::new("skilldoc-read-content");
    fixture.write_text("skills/atlas/SKILL.md", "# Atlas\n\nSkill body.");
    fixture.write_text("skills/atlas/scripts/check.ps1", "Write-Output 'ok'\n");
    fixture.write_text("skills/atlas/config/tool.toml", "name = \"atlas\"\n");
    let state = shared_state(
        fixture.path("."),
        vec![skill_ability(
            "skill:atlas",
            "atlas",
            fixture.path("skills/atlas/SKILL.md"),
        )],
    );

    let markdown = read_skill_file_state(&state, "skill:atlas", "SKILL.md").expect("read md");
    let script =
        read_skill_file_state(&state, "skill:atlas", "scripts/check.ps1").expect("read ps1");
    let config =
        read_skill_file_state(&state, "skill:atlas", "config/tool.toml").expect("read toml");

    assert_eq!(markdown.relative_path, "SKILL.md");
    assert!(markdown.content.contains("Skill body."));
    assert_eq!(script.content, "Write-Output 'ok'\n");
    assert_eq!(config.content, "name = \"atlas\"\n");
}

#[test]
fn non_skill_abilities_do_not_open_skill_detail() {
    let fixture = ScannerFixture::new("skilldoc-non-skill");
    let state = shared_state(
        fixture.path("."),
        vec![non_skill_ability("tool:shell", "shell")],
    );

    let error = open_skill_detail_window_state(&state, "tool:shell")
        .expect_err("tools must not have skill detail windows");

    assert!(error.message.contains("不是 Skill"));
}

#[test]
fn skill_detail_window_label_and_url_are_stable_and_safe() {
    let target = skill_detail_window_target("skill:unsafe id/..\\path?x=1&中");
    let label_prefix = "skill-detail-skill-unsafe-id-path-x-1-";

    assert!(target.label.starts_with(label_prefix));
    let hash_suffix = target
        .label
        .strip_prefix(label_prefix)
        .expect("label has hash suffix");
    assert_eq!(hash_suffix.len(), 16);
    assert!(hash_suffix
        .chars()
        .all(|character| character.is_ascii_hexdigit()));
    assert_eq!(
        target.app_url,
        "index.html?skillDetail=skill%3Aunsafe%20id%2F..%5Cpath%3Fx%3D1%26%E4%B8%AD"
    );
    assert!(target
        .label
        .chars()
        .all(|character| character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '-'));
    assert!(!target.app_url.contains('/'));
    assert!(!target.app_url.contains('\\'));
}

#[test]
fn skill_detail_window_labels_do_not_collide_after_sanitizing() {
    let slash_id = skill_detail_window_target("skill:a/b");
    let space_id = skill_detail_window_target("skill:a b");

    assert!(slash_id.label.starts_with("skill-detail-skill-a-b-"));
    assert!(space_id.label.starts_with("skill-detail-skill-a-b-"));
    assert_ne!(slash_id.label, space_id.label);
    assert_eq!(slash_id.app_url, "index.html?skillDetail=skill%3Aa%2Fb");
    assert_eq!(space_id.app_url, "index.html?skillDetail=skill%3Aa%20b");
}

#[test]
fn default_capability_covers_main_and_dynamic_skill_detail_windows() {
    let capability_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
    let capability: Value = serde_json::from_str(
        &std::fs::read_to_string(&capability_path).expect("read default capability"),
    )
    .expect("parse default capability");
    let windows = capability["windows"]
        .as_array()
        .expect("windows should be an array")
        .iter()
        .map(|value| value.as_str().expect("window entry should be string"))
        .collect::<Vec<_>>();
    let atlas_label = skill_detail_window_target("skill:atlas").label;
    let escaped_label = skill_detail_window_target("skill:unsafe id/..\\path?x=1&中").label;

    assert!(windows.iter().all(|pattern| *pattern != "*"));
    assert!(windows.iter().any(|pattern| glob_matches(pattern, "main")));
    assert!(windows
        .iter()
        .any(|pattern| glob_matches(pattern, &atlas_label)));
    assert!(windows
        .iter()
        .any(|pattern| glob_matches(pattern, &escaped_label)));
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut pattern_index = 0_usize;
    let mut value_index = 0_usize;
    let mut star_index = None;
    let mut match_index = 0_usize;

    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == value[value_index] || pattern[pattern_index] == b'?')
        {
            pattern_index += 1;
            value_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            match_index = value_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            match_index += 1;
            value_index = match_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}
