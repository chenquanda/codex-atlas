#[allow(dead_code)]
mod fixtures;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use codex_atlas_rust_lib::{
    commands::TranslationCommandError,
    domain::{Ability, AbilityKind},
    translate::{
        build_codex_exec_spec, cache_key, content_hash, get_translation_state,
        translate_skill_file_with_runner, TranslationCacheEntry, TranslationCommandSpec,
        TranslationError, TranslationRunner, TranslationRunnerError,
    },
};
use fixtures::ScannerFixture;
use serde_json::json;

fn skill_ability(fixture: &ScannerFixture, id: &str) -> Ability {
    Ability {
        id: id.to_string(),
        name: id.trim_start_matches("skill:").to_string(),
        kind: AbilityKind::Skill,
        path: Some(fixture.path("skills/atlas/SKILL.md")),
        summary: "English summary".to_string(),
        raw_tags: vec!["test".to_string()],
        ..Ability::default()
    }
}

fn seed_skill_file(fixture: &ScannerFixture, content: &str) {
    fixture.write_text("skills/atlas/SKILL.md", content);
}

fn seed_cache(
    project_dir: &Path,
    skill_id: &str,
    relative_path: &str,
    source_content: &str,
    translation: &str,
) {
    let hash = content_hash(source_content);
    let key = cache_key(skill_id, relative_path, &hash);
    let entry = TranslationCacheEntry {
        skill_id: skill_id.to_string(),
        relative_path: relative_path.to_string(),
        content_hash: hash,
        translation: translation.to_string(),
    };
    let path = project_dir.join("data").join("translation-cache.json");
    std::fs::create_dir_all(path.parent().expect("cache parent")).expect("create cache parent");
    std::fs::write(
        path,
        serde_json::to_string_pretty(&json!({
            "entries": BTreeMap::from([(key, entry)])
        }))
        .expect("serialize cache"),
    )
    .expect("write cache");
}

#[derive(Default)]
struct FakeRunner {
    translation: String,
    calls: Mutex<Vec<TranslationCommandSpec>>,
}

impl FakeRunner {
    fn new(translation: &str) -> Self {
        Self {
            translation: translation.to_string(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<TranslationCommandSpec> {
        self.calls.lock().expect("fake runner calls").clone()
    }
}

#[derive(Default)]
struct FailingRunner {
    calls: Mutex<Vec<TranslationCommandSpec>>,
}

impl FailingRunner {
    fn calls(&self) -> Vec<TranslationCommandSpec> {
        self.calls.lock().expect("failing runner calls").clone()
    }
}

impl TranslationRunner for FailingRunner {
    fn run(
        &self,
        spec: &TranslationCommandSpec,
        _prompt: &str,
    ) -> Result<String, TranslationRunnerError> {
        assert!(spec.input_path.is_file());
        assert!(spec.prompt_path.is_file());
        self.calls
            .lock()
            .expect("failing runner calls")
            .push(spec.clone());
        Err(TranslationRunnerError::Timeout {
            timeout_millis: spec.timeout_millis,
        })
    }
}

impl TranslationRunner for FakeRunner {
    fn run(
        &self,
        spec: &TranslationCommandSpec,
        _prompt: &str,
    ) -> Result<String, TranslationRunnerError> {
        assert!(
            spec.input_path.is_file(),
            "translation runner should receive a project .tmp input file"
        );
        assert!(
            spec.prompt_path.is_file(),
            "translation runner should receive a project .tmp prompt file"
        );
        self.calls
            .lock()
            .expect("fake runner calls")
            .push(spec.clone());
        Ok(self.translation.clone())
    }
}

#[test]
fn cache_key_uses_skill_id_relative_path_and_content_hash() {
    let hash = content_hash("Translate this skill.");
    let key = cache_key("skill:atlas", "SKILL.md", &hash);

    assert_eq!(key, cache_key("skill:atlas", "SKILL.md", &hash));
    assert_ne!(key, cache_key("skill:other", "SKILL.md", &hash));
    assert_ne!(key, cache_key("skill:atlas", "references/guide.md", &hash));
    assert_ne!(
        key,
        cache_key("skill:atlas", "SKILL.md", &content_hash("Changed content."))
    );
}

#[test]
fn cache_hit_returns_chinese_translation_without_running_command() {
    let fixture = ScannerFixture::new("translation-cache-hit");
    let ability = skill_ability(&fixture, "skill:atlas");
    let source = "# Skill\n\nTranslate this workflow.";
    seed_skill_file(&fixture, source);
    seed_cache(
        &fixture.path("."),
        "skill:atlas",
        "SKILL.md",
        source,
        "已缓存的中文译文",
    );
    let runner = FakeRunner::new("不应调用");

    let result =
        translate_skill_file_with_runner(&fixture.path("."), &ability, "SKILL.md", &runner)
            .expect("translate from cache");

    assert_eq!(result.translation, "已缓存的中文译文");
    assert!(result.cached);
    assert!(runner.calls().is_empty());
}

#[test]
fn content_hash_change_invalidates_old_cache() {
    let fixture = ScannerFixture::new("translation-cache-invalidate");
    let ability = skill_ability(&fixture, "skill:atlas");
    seed_skill_file(&fixture, "# Skill\n\nNew English content.");
    seed_cache(
        &fixture.path("."),
        "skill:atlas",
        "SKILL.md",
        "# Skill\n\nOld English content.",
        "旧缓存译文",
    );
    let runner = FakeRunner::new("新的中文译文");

    let result =
        translate_skill_file_with_runner(&fixture.path("."), &ability, "SKILL.md", &runner)
            .expect("translate changed content");

    assert_eq!(result.translation, "新的中文译文");
    assert!(!result.cached);
    assert_eq!(runner.calls().len(), 1);
}

#[test]
fn runner_failure_cleans_prompt_and_input_temp_files() {
    let fixture = ScannerFixture::new("translation-failure-cleanup");
    let ability = skill_ability(&fixture, "skill:atlas");
    seed_skill_file(&fixture, "# Skill\n\nTemporary files must be cleaned.");
    let runner = FailingRunner::default();

    let error = translate_skill_file_with_runner(&fixture.path("."), &ability, "SKILL.md", &runner)
        .expect_err("runner failure should bubble up");
    let calls = runner.calls();
    let spec = calls.first().expect("runner called");

    assert!(matches!(error, TranslationError::Command { .. }));
    assert!(!spec.input_path.exists());
    assert!(!spec.prompt_path.exists());
}

#[test]
fn chinese_content_hides_translation_action() {
    let fixture = ScannerFixture::new("translation-chinese-hidden");
    let ability = skill_ability(&fixture, "skill:atlas");
    seed_skill_file(&fixture, "# 中文 Skill\n\n这是已经本地化的说明。");

    let state =
        get_translation_state(&fixture.path("."), &ability, "SKILL.md").expect("translation state");

    assert!(!state.show_translate_action);
    assert_eq!(state.cached_translation, None);
}

#[test]
fn codex_exec_uses_project_tmp_and_timeout() {
    let fixture = ScannerFixture::new("translation-command-spec");
    let project_dir = fixture.path(".");
    let spec = build_codex_exec_spec(
        &project_dir,
        "skill:atlas",
        "references/guide.md",
        &content_hash("Guide content."),
    )
    .expect("build command spec");
    let tmp_dir = project_dir.join(".tmp");

    assert_eq!(spec.program, "codex");
    assert!(spec.args.contains(&"exec".to_string()));
    assert!(spec.args.contains(&"--ephemeral".to_string()));
    assert!(spec
        .args
        .windows(2)
        .any(|pair| { pair[0] == "-C" && PathBuf::from(&pair[1]) == project_dir }));
    assert!(spec.args.windows(2).any(|pair| {
        pair[0] == "--output-last-message" && PathBuf::from(&pair[1]) == spec.output_path
    }));
    assert_eq!(
        spec.env.get("TEMP"),
        Some(&tmp_dir.to_string_lossy().into_owned())
    );
    assert_eq!(
        spec.env.get("TMP"),
        Some(&tmp_dir.to_string_lossy().into_owned())
    );
    assert!(spec.input_path.starts_with(&tmp_dir));
    assert!(spec.prompt_path.starts_with(&tmp_dir));
    assert!(spec.output_path.starts_with(&tmp_dir));
    assert!(spec.timeout_millis > 0);
}

#[test]
fn translation_ipc_error_preserves_runner_failure_details() {
    let fixture = ScannerFixture::new("translation-structured-error");
    let project_dir = fixture.path(".");
    let spec = build_codex_exec_spec(
        &project_dir,
        "skill:atlas",
        "SKILL.md",
        &content_hash("Broken content."),
    )
    .expect("build command spec");
    let error = TranslationError::Command {
        spec,
        error: TranslationRunnerError::Failed {
            status_code: Some(7),
            stdout: "partial stdout".to_string(),
            stderr: "network unavailable".to_string(),
        },
    };

    let ipc_error = TranslationCommandError::from(error);

    assert_eq!(ipc_error.kind, "failed");
    assert_eq!(ipc_error.status_code, Some(7));
    assert_eq!(ipc_error.stdout.as_deref(), Some("partial stdout"));
    assert_eq!(ipc_error.stderr.as_deref(), Some("network unavailable"));
    assert!(ipc_error.message.contains("外部翻译命令失败"));
}
