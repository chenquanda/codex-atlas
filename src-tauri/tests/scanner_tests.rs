mod fixtures;

use codex_atlas_rust_lib::domain::AbilityKind;
use codex_atlas_rust_lib::scanner::{scan_all, ScanRoots};
use fixtures::ScannerFixture;

fn ability_names(report: &codex_atlas_rust_lib::scanner::ScanReport) -> Vec<String> {
    report
        .abilities
        .iter()
        .map(|ability| ability.name.clone())
        .collect()
}

fn ability_ids(report: &codex_atlas_rust_lib::scanner::ScanReport) -> Vec<String> {
    report
        .abilities
        .iter()
        .map(|ability| ability.id.clone())
        .collect()
}

#[test]
fn scans_user_skill_front_matter() {
    let fixture = ScannerFixture::new("front-matter");
    fixture.write_text(
        "skills/brainstorming/SKILL.md",
        r#"---
name: brainstorming
description: Explore intent before implementation
---

# Ignored Heading

This paragraph should not replace front matter.
"#,
    );
    let roots = ScanRoots::new()
        .with_skill_root(fixture.path("skills"))
        .with_plugins_root(fixture.path("plugins"))
        .with_tools_manifest(fixture.path("tools.json"))
        .with_apps_manifest(fixture.path("apps.json"))
        .with_plugins_manifest(fixture.path("plugins.json"));

    let report = scan_all(&roots);

    assert!(report.warnings.is_empty());
    assert_eq!(report.abilities.len(), 1);
    assert_eq!(report.abilities[0].id, "skill:brainstorming");
    assert_eq!(report.abilities[0].name, "brainstorming");
    assert_eq!(report.abilities[0].kind, AbilityKind::Skill);
    assert_eq!(
        report.abilities[0].summary,
        "Explore intent before implementation"
    );
}

#[test]
fn scans_skill_without_front_matter_using_directory_fallback() {
    let fixture = ScannerFixture::new("markdown-fallback");
    fixture.write_text(
        "skills/no-front-matter/SKILL.md",
        r#"# Friendly Skill

Use the first paragraph when description front matter is absent.

## Later

This should be ignored.
"#,
    );
    let roots = ScanRoots::new().with_skill_root(fixture.path("skills"));

    let report = scan_all(&roots);

    assert!(report.warnings.is_empty());
    assert_eq!(report.abilities.len(), 1);
    assert_eq!(report.abilities[0].id, "skill:no-front-matter");
    assert_eq!(report.abilities[0].name, "Friendly Skill");
    assert_eq!(
        report.abilities[0].summary,
        "Use the first paragraph when description front matter is absent."
    );
}

#[test]
fn plugin_skill_id_uses_plugin_name_prefix() {
    let fixture = ScannerFixture::new("plugin-skill-prefix");
    fixture.write_text(
        "plugins/cache/superpowers/skills/test-skill/SKILL.md",
        r#"---
name: test-skill
description: Test workflow skill
---
"#,
    );
    let roots = ScanRoots::new().with_plugins_root(fixture.path("plugins"));

    let report = scan_all(&roots);

    assert!(report.warnings.is_empty());
    assert_eq!(report.abilities.len(), 1);
    assert_eq!(report.abilities[0].id, "superpowers:test-skill");
    assert_eq!(report.abilities[0].name, "test-skill");
    assert_eq!(report.abilities[0].kind, AbilityKind::Skill);
}

#[test]
fn bad_skill_file_does_not_abort_global_scan() {
    let fixture = ScannerFixture::new("bad-file-isolation");
    fixture.write_text(
        "skills/good/SKILL.md",
        r#"---
name: good
description: Still scanned
---
"#,
    );
    fixture.write_bytes("skills/bad/SKILL.md", &[0xff, 0xfe, 0xfd]);
    let roots = ScanRoots::new().with_skill_root(fixture.path("skills"));

    let report = scan_all(&roots);

    assert_eq!(ability_names(&report), vec!["good"]);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("bad"));
    assert!(report.warnings[0].contains("SKILL.md"));
}

#[test]
fn scans_plugin_tool_and_app_catalog_entries() {
    let fixture = ScannerFixture::new("catalogs");
    fixture.write_text(
        "plugins.json",
        r#"{
  "plugins": [
    { "name": "superpowers", "description": "Workflow plugin" }
  ]
}"#,
    );
    fixture.write_text(
        "tools.json",
        r#"{
  "tools": [
    { "name": "shell", "description": "Run shell commands" }
  ]
}"#,
    );
    fixture.write_text(
        "apps.json",
        r#"{
  "apps": [
    { "name": "github", "description": "GitHub app" }
  ]
}"#,
    );
    let roots = ScanRoots::new()
        .with_plugins_manifest(fixture.path("plugins.json"))
        .with_tools_manifest(fixture.path("tools.json"))
        .with_apps_manifest(fixture.path("apps.json"));

    let report = scan_all(&roots);

    assert!(report.warnings.is_empty());
    assert_eq!(
        report
            .abilities
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
fn symlink_roots_are_skipped_when_supported() {
    let fixture = ScannerFixture::new("symlink-root");
    fixture.write_text(
        "outside/root-skill/SKILL.md",
        r#"---
name: outside-root
description: Must not be scanned through a symlink root
---
"#,
    );
    let linked_root = fixture.path("linked-root");
    if create_dir_symlink(&fixture.path("outside"), &linked_root).is_err() {
        return;
    }
    let roots = ScanRoots::new().with_skill_root(linked_root);

    let report = scan_all(&roots);

    assert!(
        report.abilities.is_empty(),
        "symlink root must not be followed: {:?}",
        ability_ids(&report)
    );
}

#[test]
fn symlink_skill_files_are_skipped_when_supported() {
    let fixture = ScannerFixture::new("symlink-skill-file");
    fixture.write_text(
        "outside/SKILL.md",
        r#"---
name: outside-file
description: Must not be scanned through a symlink SKILL.md
---
"#,
    );
    std::fs::create_dir_all(fixture.path("skills/linked-file"))
        .expect("create symlink skill parent");
    if create_file_symlink(
        &fixture.path("outside/SKILL.md"),
        &fixture.path("skills/linked-file/SKILL.md"),
    )
    .is_err()
    {
        return;
    }
    let roots = ScanRoots::new().with_skill_root(fixture.path("skills"));

    let report = scan_all(&roots);

    assert!(
        report.abilities.is_empty(),
        "symlink SKILL.md must not be followed: {:?}",
        ability_ids(&report)
    );
}

#[test]
fn symlink_plugin_skills_dirs_are_skipped_when_supported() {
    let fixture = ScannerFixture::new("symlink-plugin-skills-dir");
    fixture.write_text(
        "outside-skills/external/SKILL.md",
        r#"---
name: outside-plugin-skill
description: Must not be scanned through a symlink skills dir
---
"#,
    );
    std::fs::create_dir_all(fixture.path("plugins/cache/superpowers"))
        .expect("create plugin parent");
    if create_dir_symlink(
        &fixture.path("outside-skills"),
        &fixture.path("plugins/cache/superpowers/skills"),
    )
    .is_err()
    {
        return;
    }
    let roots = ScanRoots::new().with_plugins_root(fixture.path("plugins"));

    let report = scan_all(&roots);

    assert!(
        report.abilities.is_empty(),
        "symlink plugin skills dir must not be followed: {:?}",
        ability_ids(&report)
    );
}

#[test]
fn versioned_plugin_cache_uses_plugin_name_not_version() {
    let fixture = ScannerFixture::new("versioned-plugin-cache");
    fixture.write_text(
        "plugins/cache/openai-curated-remote/superpowers/5.1.4/skills/test-skill/SKILL.md",
        r#"---
name: test-skill
description: Versioned cache skill
---
"#,
    );
    let roots = ScanRoots::new().with_plugins_root(fixture.path("plugins"));

    let report = scan_all(&roots);

    assert!(report.warnings.is_empty());
    assert_eq!(ability_ids(&report), vec!["superpowers:test-skill"]);
}

#[test]
fn ability_ids_are_normalized_and_duplicates_warn() {
    let fixture = ScannerFixture::new("normalized-duplicates");
    fixture.write_text(
        "skills/first/SKILL.md",
        r#"---
name: Brain Storm/Win\Do
description: First duplicate wins
---
"#,
    );
    fixture.write_text(
        "skills/second/SKILL.md",
        r#"---
name: brain   storm win do
description: Should be skipped as duplicate
---
"#,
    );
    fixture.write_text(
        "tools.json",
        r#"{
  "tools": [
    { "name": "Shell Tool", "description": "First tool wins" },
    { "name": "shell/tool", "description": "Duplicate tool should warn" }
  ]
}"#,
    );
    let roots = ScanRoots::new()
        .with_skill_root(fixture.path("skills"))
        .with_tools_manifest(fixture.path("tools.json"));

    let report = scan_all(&roots);

    assert_eq!(
        ability_ids(&report),
        vec!["skill:brain-storm-win-do", "tool:shell-tool"]
    );
    assert_eq!(report.abilities[0].summary, "First duplicate wins");
    assert_eq!(report.abilities[1].summary, "First tool wins");
    assert_eq!(report.warnings.len(), 2);
    assert!(report
        .warnings
        .iter()
        .all(|warning| warning.contains("重复 ability id")));
}

#[test]
fn front_matter_multiline_description_is_used_as_summary() {
    let fixture = ScannerFixture::new("multiline-front-matter");
    fixture.write_text(
        "skills/folded/SKILL.md",
        r#"---
name: folded
description: >
  First folded line
  second folded line
---
"#,
    );
    fixture.write_text(
        "skills/literal/SKILL.md",
        r#"---
name: literal
description: |
  First literal line
  second literal line
---
"#,
    );
    let roots = ScanRoots::new().with_skill_root(fixture.path("skills"));

    let report = scan_all(&roots);

    assert!(report.warnings.is_empty());
    assert_eq!(report.abilities.len(), 2);
    assert_eq!(
        report
            .abilities
            .iter()
            .map(|ability| ability.summary.as_str())
            .collect::<Vec<_>>(),
        vec![
            "First folded line second folded line",
            "First literal line\nsecond literal line"
        ]
    );
}

#[test]
fn too_deep_skill_directories_are_warned_and_skipped() {
    let fixture = ScannerFixture::new("max-depth");
    fixture.write_text(
        "skills/d1/d2/d3/d4/d5/d6/d7/d8/d9/too-deep/SKILL.md",
        r#"---
name: too-deep
description: Should be skipped past max depth
---
"#,
    );
    let roots = ScanRoots::new().with_skill_root(fixture.path("skills"));

    let report = scan_all(&roots);

    assert!(report.abilities.is_empty());
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("超过最大扫描深度"));
}

#[test]
fn oversized_skill_file_is_warned_and_skipped() {
    let fixture = ScannerFixture::new("oversized");
    let oversized = "x".repeat(512 * 1024 + 1);
    fixture.write_text("skills/huge/SKILL.md", &oversized);
    let roots = ScanRoots::new().with_skill_root(fixture.path("skills"));

    let report = scan_all(&roots);

    assert!(report.abilities.is_empty());
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("超过最大文件大小"));
}

#[test]
fn bad_manifest_entry_does_not_drop_valid_siblings() {
    let fixture = ScannerFixture::new("manifest-entry-isolation");
    fixture.write_text(
        "tools.json",
        r#"{
  "tools": [
    { "description": "Missing name should warn" },
    { "name": "shell", "description": "Run shell commands" }
  ]
}"#,
    );
    let roots = ScanRoots::new().with_tools_manifest(fixture.path("tools.json"));

    let report = scan_all(&roots);

    assert_eq!(ability_ids(&report), vec!["tool:shell"]);
    assert_eq!(report.abilities[0].summary, "Run shell commands");
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("缺少 name"));
}

#[test]
fn id_parts_replace_colons_with_dashes() {
    let fixture = ScannerFixture::new("colon-id-parts");
    fixture.write_text(
        "skills/user/SKILL.md",
        r#"---
name: Foo:Bar
description: User skill colon should not add an id segment
---
"#,
    );
    fixture.write_text(
        "plugins/cache/superpowers/skills/plugin-skill/SKILL.md",
        r#"---
name: Plugin:Skill
description: Plugin skill colon should not add an id segment
---
"#,
    );
    fixture.write_text(
        "plugins.json",
        r#"{
  "plugins": [
    { "name": "Plugin:Catalog", "description": "Catalog colon should not add an id segment" }
  ]
}"#,
    );
    let roots = ScanRoots::new()
        .with_skill_root(fixture.path("skills"))
        .with_plugins_root(fixture.path("plugins"))
        .with_plugins_manifest(fixture.path("plugins.json"));

    let report = scan_all(&roots);

    assert!(report.warnings.is_empty());
    assert_eq!(
        ability_ids(&report),
        vec![
            "skill:foo-bar",
            "superpowers:plugin-skill",
            "plugin:plugin-catalog"
        ]
    );
    assert!(ability_ids(&report)
        .iter()
        .all(|id| id.matches(':').count() == 1));
}

#[test]
fn manifest_symlinks_are_skipped_when_supported() {
    let fixture = ScannerFixture::new("manifest-symlink");
    fixture.write_text(
        "outside/tools.json",
        r#"{
  "tools": [
    { "name": "external", "description": "Must not be read through symlink manifest" }
  ]
}"#,
    );
    let linked_manifest = fixture.path("tools-link.json");
    if create_file_symlink(&fixture.path("outside/tools.json"), &linked_manifest).is_err() {
        return;
    }
    let roots = ScanRoots::new().with_tools_manifest(linked_manifest);

    let report = scan_all(&roots);

    assert!(
        report.abilities.is_empty(),
        "symlink manifest must not be followed: {:?}",
        ability_ids(&report)
    );
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("symlink"));
}

#[test]
fn oversized_manifest_is_warned_and_skipped() {
    let fixture = ScannerFixture::new("oversized-manifest");
    fixture.write_text("tools.json", &"x".repeat(512 * 1024 + 1));
    let roots = ScanRoots::new().with_tools_manifest(fixture.path("tools.json"));

    let report = scan_all(&roots);

    assert!(report.abilities.is_empty());
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("manifest 超过最大文件大小"));
}

#[test]
fn front_matter_block_scalar_chomping_indicators_are_accepted() {
    let fixture = ScannerFixture::new("front-matter-chomping");
    fixture.write_text(
        "skills/folded-strip/SKILL.md",
        r#"---
name: folded-strip
description: >-
  Folded strip line
  second line
---
"#,
    );
    fixture.write_text(
        "skills/literal-keep/SKILL.md",
        r#"---
name: literal-keep
description: |+
  Literal keep line
  second line
---
"#,
    );
    let roots = ScanRoots::new().with_skill_root(fixture.path("skills"));

    let report = scan_all(&roots);

    assert!(report.warnings.is_empty());
    assert_eq!(
        report
            .abilities
            .iter()
            .map(|ability| ability.summary.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Folded strip line second line",
            "Literal keep line\nsecond line"
        ]
    );
}

#[cfg(unix)]
fn create_dir_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_dir_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(unix)]
fn create_file_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_symlink(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}
