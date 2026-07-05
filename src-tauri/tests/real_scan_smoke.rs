use std::{env, path::PathBuf};

use codex_atlas_rust_lib::{
    domain::AbilityKind,
    scanner::{scan_all, ScanRoots},
};

const SKILL_ROOTS_ENV: &str = "CODEX_ATLAS_SMOKE_SKILL_ROOTS";
const PLUGIN_ROOTS_ENV: &str = "CODEX_ATLAS_SMOKE_PLUGIN_ROOTS";

#[test]
fn real_codex_roots_scan_at_least_one_skill_when_present() {
    let roots = scan_roots_from_env();
    if roots.skill_roots.is_empty() && roots.plugins_roots.is_empty() {
        eprintln!("跳过：脚本未传入真实 Codex roots。");
        return;
    }

    let report = scan_all(&roots);
    let skill_abilities = report
        .abilities
        .iter()
        .filter(|ability| ability.kind == AbilityKind::Skill)
        .collect::<Vec<_>>();

    assert!(
        !skill_abilities.is_empty(),
        "真实 roots 应至少扫描出一个 Skill 或 plugin skill；abilities={:?}; warnings={:?}",
        report
            .abilities
            .iter()
            .map(|ability| (&ability.id, &ability.kind, &ability.path))
            .collect::<Vec<_>>(),
        report.warnings
    );

    assert!(
        skill_abilities.iter().all(|ability| ability.path.is_some()),
        "真实扫描返回的 Skill 应保留只读来源路径，便于后续详情阅读"
    );
}

fn scan_roots_from_env() -> ScanRoots {
    let mut roots = ScanRoots::new();

    roots.skill_roots.extend(env_paths(SKILL_ROOTS_ENV));
    roots.plugins_roots.extend(env_paths(PLUGIN_ROOTS_ENV));

    roots
}

fn env_paths(name: &str) -> Vec<PathBuf> {
    env::var_os(name)
        .map(|raw| {
            env::split_paths(&raw)
                .filter(|path| !path.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default()
}
