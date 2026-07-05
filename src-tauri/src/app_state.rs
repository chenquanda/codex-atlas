use std::{
    fs, io,
    path::Path,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use crate::{
    scanner::ScanRoots,
    stats::StatsRoots,
    store::{load_stats_cache_report, load_store, Store},
};

pub const STORE_FILE: &str = "store.json";
const MAX_DEFAULT_STATS_FILES: usize = 512;
const MAX_DEFAULT_STATS_DEPTH: usize = 8;
const DEFAULT_PLUGIN_MANIFESTS: &[&str] = &[".codex/plugins/plugins.json", ".codex/plugins.json"];
const DEFAULT_TOOL_MANIFESTS: &[&str] = &[".codex/tools/tools.json", ".codex/tools.json"];
const DEFAULT_APP_MANIFESTS: &[&str] = &[".codex/apps/apps.json", ".codex/apps.json"];

pub type SharedAppState = Arc<RwLock<AppState>>;

#[derive(Debug)]
pub struct AppState {
    pub project_dir: PathBuf,
    pub scan_roots: ScanRoots,
    pub stats_roots: StatsRoots,
    pub store: Store,
    pub warnings: Vec<String>,
}

impl AppState {
    pub fn new(project_dir: PathBuf, scan_roots: ScanRoots, stats_roots: StatsRoots) -> Self {
        let mut warnings = Vec::new();
        let mut store = match load_store(&project_dir, STORE_FILE) {
            Ok(result) => {
                warnings.extend(result.warnings);
                result.store
            }
            Err(error) => {
                warnings.push(format!("加载存储失败，已使用空存储: {error}"));
                Store::default()
            }
        };

        match load_stats_cache_report(&project_dir) {
            Ok(result) => {
                warnings.extend(result.warnings);
                for (id, stats) in result.stats {
                    store.stats.entry(id).or_insert(stats);
                }
            }
            Err(error) => warnings.push(format!("加载统计缓存失败，已忽略缓存: {error}")),
        }

        Self {
            project_dir,
            scan_roots,
            stats_roots,
            store,
            warnings,
        }
    }
}

pub fn default_scan_roots_for_home(home: &Path) -> ScanRoots {
    let mut roots = ScanRoots::new()
        .with_skill_root(home.join(".codex").join("skills"))
        .with_skill_root(home.join(".agents").join("skills"))
        .with_plugins_root(home.join(".codex").join("plugins"));

    collect_existing_default_manifests(
        home,
        DEFAULT_PLUGIN_MANIFESTS,
        &mut roots.plugins_manifests,
    );
    collect_existing_default_manifests(home, DEFAULT_TOOL_MANIFESTS, &mut roots.tools_manifests);
    collect_existing_default_manifests(home, DEFAULT_APP_MANIFESTS, &mut roots.apps_manifests);

    roots
}

pub fn default_stats_roots_for_home(home: &Path) -> StatsRoots {
    let mut discovery = StatsDiscovery::default();

    // 默认统计只读收集常见 Codex session JSONL，并设置深度和数量上限；
    // discovery 的截断、读目录错误和边界跳过会随 StatsRoots 传给统计层，防止把不完整文件集当成可信 0 保存。
    collect_jsonl_root(&home.join(".codex").join("sessions"), &mut discovery);
    discovery.files.sort();

    StatsRoots {
        conversation_files: discovery.files,
        discovery_complete: discovery.complete,
        discovery_warnings: discovery.warnings,
    }
}

pub fn default_shared_app_state(project_dir: PathBuf) -> SharedAppState {
    let home = default_home_dir().unwrap_or_else(|| project_dir.clone());
    shared_app_state(
        project_dir,
        default_scan_roots_for_home(&home),
        default_stats_roots_for_home(&home),
    )
}

pub fn shared_app_state(
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

fn default_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            Some(PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                path.to_string_lossy()
            )))
        })
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

fn collect_existing_default_manifests(
    home: &Path,
    candidates: &[&str],
    manifests: &mut Vec<PathBuf>,
) {
    for candidate in candidates {
        let path = home.join(Path::new(candidate));
        if is_existing_regular_file(&path) {
            manifests.push(path);
        }
    }
}

fn is_existing_regular_file(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(metadata) => metadata.is_file() && !metadata.file_type().is_symlink(),
        Err(_) => false,
    }
}

#[derive(Debug)]
struct StatsDiscovery {
    files: Vec<PathBuf>,
    complete: bool,
    warnings: Vec<String>,
}

impl Default for StatsDiscovery {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            complete: true,
            warnings: Vec::new(),
        }
    }
}

fn collect_jsonl_root(root: &Path, discovery: &mut StatsDiscovery) {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            mark_stats_discovery_incomplete(
                discovery,
                format!("默认统计根目录不存在，已跳过: {}", root.display()),
            );
            return;
        }
        Err(error) => {
            mark_stats_discovery_incomplete(
                discovery,
                format!("读取默认统计根目录元数据失败 {}: {error}", root.display()),
            );
            return;
        }
    };

    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        mark_stats_discovery_incomplete(
            discovery,
            format!("跳过非真实目录默认统计根目录: {}", root.display()),
        );
        return;
    }

    let canonical_root = match root.canonicalize() {
        Ok(canonical) => canonical,
        Err(error) => {
            mark_stats_discovery_incomplete(
                discovery,
                format!("解析默认统计根目录真实路径失败 {}: {error}", root.display()),
            );
            return;
        }
    };

    collect_jsonl_files(&canonical_root, &canonical_root, 0, discovery);
}

fn collect_jsonl_files(
    canonical_root: &Path,
    directory: &Path,
    depth: usize,
    discovery: &mut StatsDiscovery,
) {
    if depth > MAX_DEFAULT_STATS_DEPTH {
        mark_stats_discovery_incomplete(
            discovery,
            format!(
                "默认统计 discovery 超过最大深度，跳过: {}",
                directory.display()
            ),
        );
        return;
    }

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            mark_stats_discovery_incomplete(
                discovery,
                format!("无法读取默认统计目录 {}: {error}", directory.display()),
            );
            return;
        }
    };

    let mut entries = entries
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry),
            Err(error) => {
                mark_stats_discovery_incomplete(
                    discovery,
                    format!("读取默认统计目录项失败 {}: {error}", directory.display()),
                );
                None
            }
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        if discovery.files.len() >= MAX_DEFAULT_STATS_FILES {
            mark_stats_discovery_incomplete(
                discovery,
                format!(
                    "默认统计 discovery 达到数量上限 {MAX_DEFAULT_STATS_FILES}，已跳过后续文件"
                ),
            );
            break;
        }

        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                mark_stats_discovery_incomplete(
                    discovery,
                    format!("读取默认统计路径元数据失败 {}: {error}", path.display()),
                );
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            mark_stats_discovery_incomplete(
                discovery,
                format!("跳过 symlink 默认统计路径: {}", path.display()),
            );
            continue;
        }

        let canonical_child = match path.canonicalize() {
            Ok(canonical) => canonical,
            Err(error) => {
                mark_stats_discovery_incomplete(
                    discovery,
                    format!("解析默认统计路径真实路径失败 {}: {error}", path.display()),
                );
                continue;
            }
        };
        if !canonical_child.starts_with(canonical_root) {
            mark_stats_discovery_incomplete(
                discovery,
                format!(
                    "默认统计路径越过根目录，跳过: {}",
                    canonical_child.display()
                ),
            );
            continue;
        }

        if metadata.is_dir() {
            if depth + 1 > MAX_DEFAULT_STATS_DEPTH {
                mark_stats_discovery_incomplete(
                    discovery,
                    format!(
                        "默认统计 discovery 超过最大深度，跳过: {}",
                        canonical_child.display()
                    ),
                );
                continue;
            }
            collect_jsonl_files(canonical_root, &canonical_child, depth + 1, discovery);
            continue;
        }
        if metadata.is_file()
            && canonical_child
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.eq_ignore_ascii_case("jsonl"))
                .unwrap_or(false)
        {
            discovery.files.push(canonical_child);
        }
    }
}

fn mark_stats_discovery_incomplete(discovery: &mut StatsDiscovery, warning: String) {
    discovery.complete = false;
    if !discovery.warnings.contains(&warning) {
        discovery.warnings.push(warning);
    }
}
