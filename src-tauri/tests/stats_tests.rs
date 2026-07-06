#[allow(dead_code)]
mod fixtures;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use codex_atlas_rust_lib::{
    domain::{format_usage, Ability, AbilityKind, Stats},
    stats::{refresh_usage_stats, AbilityUsageIndex, StatsRoots},
    store::{load_stats_cache, load_stats_cache_report, save_stats_cache},
};
use fixtures::ScannerFixture;
use serde_json::{json, Value};

fn skill(id: &str, name: &str, path: PathBuf) -> Ability {
    Ability {
        id: id.to_string(),
        name: name.to_string(),
        kind: AbilityKind::Skill,
        path: Some(path),
        ..Ability::default()
    }
}

fn report_count(
    report: &codex_atlas_rust_lib::stats::StatsReport,
    ability_id: &str,
) -> Option<u64> {
    report
        .stats
        .get(ability_id)
        .and_then(|stats| stats.usage_count)
}

fn jsonl(records: &[Value]) -> String {
    let mut lines = records
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    lines.push('\n');
    lines
}

#[test]
fn counts_explicit_user_skill_mentions_repeatedly() {
    let fixture = ScannerFixture::new("stats-user-skill-repeated");
    let ability = skill(
        "skill:brainstorming",
        "brainstorming",
        fixture.path("skills/brainstorming/SKILL.md"),
    );
    fixture.write_text(
        "conversations/session.jsonl",
        r#"{"role":"user","content":"先用 $brainstorming，再用 $brainstorming。"}
{"role":"user","content":"下一轮继续 $brainstorming；$brainstorming-extra 不是同一个 skill。"}
"#,
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert!(report.warnings.is_empty());
    assert_eq!(report_count(&report, "skill:brainstorming"), Some(3));
}

#[test]
fn counts_plugin_skill_mentions_repeatedly() {
    let fixture = ScannerFixture::new("stats-plugin-skill-repeated");
    let ability = skill(
        "superpowers:test-driven-development",
        "test-driven-development",
        fixture.path("plugins/superpowers/skills/test-driven-development/SKILL.md"),
    );
    fixture.write_text(
        "conversations/session.jsonl",
        r#"{"role":"user","content":"请走 $superpowers:test-driven-development，然后复核 $superpowers:test-driven-development。"}
{"role":"user","content":"单独的 $test-driven-development 不应该算到插件 skill。"}
"#,
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert!(report.warnings.is_empty());
    assert_eq!(
        report_count(&report, "superpowers:test-driven-development"),
        Some(2)
    );
}

#[test]
fn counts_explicit_mentions_from_real_payload_envelope_records() {
    let fixture = ScannerFixture::new("stats-payload-user-mentions");
    let abilities = vec![
        skill(
            "skill:brainstorming",
            "brainstorming",
            fixture.path("skills/brainstorming/SKILL.md"),
        ),
        skill(
            "superpowers:test-driven-development",
            "test-driven-development",
            fixture.path("plugins/superpowers/skills/test-driven-development/SKILL.md"),
        ),
    ];
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "type": "message",
                "payload": {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "先用 $brainstorming。"
                        }
                    ],
                },
            }),
            json!({
                "type": "message",
                "payload": {
                    "message": {
                        "role": "user",
                        "content": [
                            {
                                "type": "text",
                                "text": "然后走 $superpowers:test-driven-development。"
                            }
                        ],
                    },
                },
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&abilities);

    let report = refresh_usage_stats(&roots, &index);

    assert!(report.warnings.is_empty());
    assert_eq!(report_count(&report, "skill:brainstorming"), Some(1));
    assert_eq!(
        report_count(&report, "superpowers:test-driven-development"),
        Some(1)
    );
}

#[test]
fn counts_assistant_skill_file_reads() {
    let fixture = ScannerFixture::new("stats-assistant-read");
    let skill_path = fixture.path("skills/brainstorming/SKILL.md");
    let ability = skill("skill:brainstorming", "brainstorming", skill_path.clone());
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "role": "assistant",
                "content": format!("Read file {} before planning.", skill_path.display()),
            }),
            json!({
                "role": "assistant",
                "content": "Available skills include $brainstorming, but this line is only a list.",
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert!(report.warnings.is_empty());
    assert_eq!(report_count(&report, "skill:brainstorming"), Some(1));
}

#[test]
fn counts_assistant_skill_reads_from_real_payload_envelope_records() {
    let fixture = ScannerFixture::new("stats-payload-assistant-read");
    let skill_path = fixture.path("skills/brainstorming/SKILL.md");
    let plugin_path = fixture.path("plugins/superpowers/skills/test-driven-development/SKILL.md");
    let abilities = vec![
        skill("skill:brainstorming", "brainstorming", skill_path.clone()),
        skill(
            "superpowers:test-driven-development",
            "test-driven-development",
            plugin_path.clone(),
        ),
    ];
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "type": "message",
                "payload": {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": format!("Read file {}", skill_path.display()),
                        }
                    ],
                },
            }),
            json!({
                "type": "message",
                "payload": {
                    "message": {
                        "role": "assistant",
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Read file {}", plugin_path.display()),
                            }
                        ],
                    },
                },
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&abilities);

    let report = refresh_usage_stats(&roots, &index);

    assert!(report.warnings.is_empty());
    assert_eq!(report_count(&report, "skill:brainstorming"), Some(1));
    assert_eq!(
        report_count(&report, "superpowers:test-driven-development"),
        Some(1)
    );
}

#[test]
fn ignores_system_developer_tool_and_function_noise() {
    let fixture = ScannerFixture::new("stats-role-noise");
    let skill_path = fixture.path("skills/brainstorming/SKILL.md");
    let plugin_path = fixture.path("plugins/superpowers/skills/test-driven-development/SKILL.md");
    let abilities = vec![
        skill("skill:brainstorming", "brainstorming", skill_path.clone()),
        skill(
            "superpowers:test-driven-development",
            "test-driven-development",
            plugin_path.clone(),
        ),
    ];
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "role": "system",
                "content": format!("Skills list: $brainstorming $brainstorming {}", skill_path.display()),
            }),
            json!({
                "role": "developer",
                "content": "Use $superpowers:test-driven-development when coding.",
            }),
            json!({
                "role": "tool",
                "content": format!("Read file {} and mention $brainstorming.", plugin_path.display()),
            }),
            json!({
                "role": "function",
                "content": "$brainstorming $superpowers:test-driven-development",
            }),
            json!({
                "role": "assistant",
                "content": "Available skills: $brainstorming and $superpowers:test-driven-development.",
            }),
            json!({
                "role": "user",
                "content": "brainstorming and superpowers:test-driven-development are bare words only.",
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&abilities);

    let report = refresh_usage_stats(&roots, &index);

    assert!(report.warnings.is_empty());
    assert_eq!(report_count(&report, "skill:brainstorming"), Some(0));
    assert_eq!(
        report_count(&report, "superpowers:test-driven-development"),
        Some(0)
    );
}

#[test]
fn ignores_noise_roles_inside_real_payload_envelope_records() {
    let fixture = ScannerFixture::new("stats-payload-role-noise");
    let skill_path = fixture.path("skills/brainstorming/SKILL.md");
    let ability = skill("skill:brainstorming", "brainstorming", skill_path.clone());
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "type": "message",
                "payload": {
                    "role": "system",
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Read file {} and mention $brainstorming", skill_path.display()),
                        }
                    ],
                },
            }),
            json!({
                "type": "message",
                "payload": {
                    "role": "developer",
                    "content": "$brainstorming",
                },
            }),
            json!({
                "type": "message",
                "payload": {
                    "message": {
                        "role": "tool",
                        "content": format!("Read file {}", skill_path.display()),
                    },
                },
            }),
            json!({
                "type": "message",
                "payload": {
                    "message": {
                        "author": {
                            "role": "function",
                        },
                        "content": "$brainstorming",
                    },
                },
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert!(report.warnings.is_empty());
    assert_eq!(report_count(&report, "skill:brainstorming"), Some(0));
}

#[test]
fn unknown_stats_remain_unknown_when_cache_missing() {
    let fixture = ScannerFixture::new("stats-cache-missing");

    let cache = load_stats_cache(fixture.path(".")).expect("missing cache should load as empty");
    let unknown = cache.get("skill:unknown").cloned().unwrap_or_default();

    assert!(cache.is_empty());
    assert_eq!(
        unknown,
        Stats {
            usage_count: None,
            last_used_at: None,
        }
    );
    assert_eq!(format_usage(unknown.usage_count), "未统计");
}

#[test]
fn stats_cache_roundtrips_under_project_data_dir() {
    let fixture = ScannerFixture::new("stats-cache-roundtrip");
    let mut stats = BTreeMap::new();
    stats.insert(
        "skill:brainstorming".to_string(),
        Stats {
            usage_count: Some(4),
            last_used_at: None,
        },
    );

    save_stats_cache(fixture.path("."), &stats).expect("save stats cache");
    let loaded = load_stats_cache(fixture.path(".")).expect("load stats cache");

    assert_eq!(loaded, stats);
    assert!(fixture
        .path(Path::new("data").join("stats-cache.json"))
        .exists());
}

#[test]
fn bad_jsonl_line_warns_and_scan_continues() {
    let fixture = ScannerFixture::new("stats-bad-jsonl");
    let ability = skill(
        "skill:brainstorming",
        "brainstorming",
        fixture.path("skills/brainstorming/SKILL.md"),
    );
    fixture.write_text(
        "conversations/session.jsonl",
        r#"{"role":"user","content":"先 $brainstorming。"}
{ bad json
{"role":"user","content":"再 $brainstorming。"}
"#,
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:brainstorming"), Some(2));
    assert!(!report.complete);
    assert!(!report.can_save_cache());
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("JSONL"));
}

#[test]
fn missing_conversation_file_marks_report_incomplete_and_unsaveable() {
    let fixture = ScannerFixture::new("stats-missing-conversation");
    let ability = skill(
        "skill:brainstorming",
        "brainstorming",
        fixture.path("skills/brainstorming/SKILL.md"),
    );
    fixture.write_text(
        "conversations/readable.jsonl",
        r#"{"role":"user","content":"已读取文件里的 $brainstorming 仍然保留。"}
"#,
    );

    let roots = StatsRoots {
        conversation_files: vec![
            fixture.path("conversations/readable.jsonl"),
            fixture.path("conversations/missing.jsonl"),
        ],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert!(!report.complete);
    assert!(!report.can_save_cache());
    assert_eq!(report_count(&report, "skill:brainstorming"), Some(1));
    assert!(!report.warnings.is_empty());
}

#[test]
fn assistant_skill_reads_require_read_signal_and_component_suffix_match() {
    let fixture = ScannerFixture::new("stats-read-boundaries");
    let skill_path = fixture.path("skills/tdd/SKILL.md");
    let sibling_path = fixture.path("skills/my-tdd/SKILL.md");
    let ability = skill("skill:tdd", "tdd", skill_path.clone());
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "role": "assistant",
                "content": format!("Read file {}", sibling_path.display()),
            }),
            json!({
                "role": "assistant",
                "content": format!("Available skills: {}", skill_path.display()),
            }),
            json!({
                "role": "assistant",
                "content": format!("Read file {}", skill_path.display()),
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:tdd"), Some(1));
}

#[test]
fn assistant_skill_reads_disambiguate_same_skill_names_by_source_path() {
    let fixture = ScannerFixture::new("stats-source-path-disambiguation");
    let user_path = fixture.path("skills/foo/SKILL.md");
    let plugin_path = fixture.path("plugins/superpowers/skills/foo/SKILL.md");
    let abilities = vec![
        skill("skill:foo", "foo", user_path.clone()),
        skill("superpowers:foo", "foo", plugin_path.clone()),
    ];
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "role": "assistant",
                "content": format!("Read file {}", user_path.display()),
            }),
            json!({
                "role": "assistant",
                "content": format!("Read file {}", plugin_path.display()),
            }),
            json!({
                "role": "assistant",
                "content": "Read file foo/SKILL.md",
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&abilities);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:foo"), Some(1));
    assert_eq!(report_count(&report, "superpowers:foo"), Some(1));
}

#[test]
fn assistant_plugin_skill_path_does_not_match_user_skill_marker() {
    let fixture = ScannerFixture::new("stats-plugin-path-does-not-match-user");
    let ability = skill("skill:foo", "foo", fixture.path("skills/foo/SKILL.md"));
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[json!({
            "role": "assistant",
            "content": format!(
                "Read file {}",
                fixture
                    .path("plugins/superpowers/skills/foo/SKILL.md")
                    .display()
            ),
        })]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:foo"), Some(0));
}

#[test]
fn assistant_direct_plugin_skill_candidate_does_not_match_user_short_marker() {
    let fixture = ScannerFixture::new("stats-direct-plugin-candidate");
    let ability = skill("skill:foo", "foo", fixture.path("skills/foo/SKILL.md"));
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "role": "assistant",
                "content": "Read file superpowers/skills/foo/SKILL.md",
            }),
            json!({
                "role": "assistant",
                "content": "Read file skills/foo/SKILL.md",
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:foo"), Some(1));
}

#[test]
fn assistant_structured_tool_call_paths_count_as_actual_reads() {
    let fixture = ScannerFixture::new("stats-structured-tool-call");
    let skill_path = fixture.path("skills/brainstorming/SKILL.md");
    let ability = skill("skill:brainstorming", "brainstorming", skill_path.clone());
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "role": "assistant",
                "input": {
                    "path": skill_path,
                },
            }),
            json!({
                "role": "assistant",
                "tool_calls": [{
                    "function": {
                        "arguments": json!({
                            "cmd": format!("Get-Content -LiteralPath '{}'", fixture.path("skills/brainstorming/SKILL.md").display()),
                        }).to_string(),
                    },
                }],
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:brainstorming"), Some(2));
}

#[test]
fn user_mentions_handle_punctuation_and_skip_code_or_money() {
    let fixture = ScannerFixture::new("stats-mention-boundaries");
    let abilities = vec![
        skill(
            "skill:brainstorming",
            "brainstorming",
            fixture.path("skills/brainstorming/SKILL.md"),
        ),
        skill(
            "superpowers:test-driven-development",
            "test-driven-development",
            fixture.path("plugins/superpowers/skills/test-driven-development/SKILL.md"),
        ),
    ];
    fixture.write_text(
        "conversations/session.jsonl",
        r#"{"role":"user","content":"请用 $brainstorming. 然后 $superpowers:test-driven-development.\n内联 `$brainstorming` 不算。\n```sh\n$brainstorming\n$superpowers:test-driven-development\n```\n预算是 $10，不是 skill。"}
"#,
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&abilities);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:brainstorming"), Some(1));
    assert_eq!(
        report_count(&report, "superpowers:test-driven-development"),
        Some(1)
    );
}

#[test]
fn user_mentions_skip_double_backtick_inline_code() {
    let fixture = ScannerFixture::new("stats-mention-code-colon");
    let ability = skill(
        "skill:brainstorming",
        "brainstorming",
        fixture.path("skills/brainstorming/SKILL.md"),
    );
    fixture.write_text(
        "conversations/session.jsonl",
        r#"{"role":"user","content":"双反引号 ``$brainstorming`` 不算。"}
"#,
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:brainstorming"), Some(0));
}

#[test]
fn user_mentions_accept_trailing_colon() {
    let fixture = ScannerFixture::new("stats-mention-trailing-colon");
    let ability = skill(
        "skill:brainstorming",
        "brainstorming",
        fixture.path("skills/brainstorming/SKILL.md"),
    );
    fixture.write_text(
        "conversations/session.jsonl",
        r#"{"role":"user","content":"命令 $brainstorming: 要算。"}
"#,
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert_eq!(report_count(&report, "skill:brainstorming"), Some(1));
}

#[test]
fn bad_stats_cache_json_degrades_to_empty_with_warning() {
    let fixture = ScannerFixture::new("stats-cache-bad-json");
    fixture.write_text("data/stats-cache.json", "{ bad json");

    let result = load_stats_cache_report(fixture.path(".")).expect("bad cache should degrade");
    let legacy_map = load_stats_cache(fixture.path(".")).expect("legacy cache API should degrade");

    assert!(result.stats.is_empty());
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("stats-cache"));
    assert!(legacy_map.is_empty());
}

#[test]
fn nested_system_developer_tool_and_function_roles_are_noise() {
    let fixture = ScannerFixture::new("stats-nested-role-noise");
    let skill_path = fixture.path("skills/brainstorming/SKILL.md");
    let ability = skill("skill:brainstorming", "brainstorming", skill_path.clone());
    fixture.write_text(
        "conversations/session.jsonl",
        &jsonl(&[
            json!({
                "message": {
                    "role": "system",
                    "content": format!("Read file {} and $brainstorming", skill_path.display()),
                },
            }),
            json!({
                "message": {
                    "author": { "role": "developer" },
                    "content": "$brainstorming",
                },
            }),
            json!({
                "message": {
                    "role": "tool",
                    "content": format!("Read file {}", skill_path.display()),
                },
            }),
            json!({
                "message": {
                    "author": { "role": "function" },
                    "content": "$brainstorming",
                },
            }),
        ]),
    );

    let roots = StatsRoots {
        conversation_files: vec![fixture.path("conversations/session.jsonl")],
        ..StatsRoots::default()
    };
    let index = AbilityUsageIndex::from_abilities(&[ability]);

    let report = refresh_usage_stats(&roots, &index);

    assert!(report.warnings.is_empty());
    assert_eq!(report_count(&report, "skill:brainstorming"), Some(0));
}
