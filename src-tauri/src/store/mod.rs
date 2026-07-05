use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    domain::{merge_layers_with_stats, AIData, Ability, Stats, UserData},
    util::{self, DataPathError},
};

pub const STATS_CACHE_FILE: &str = "stats-cache.json";

/// 存储持久化约定：abilities 只保存扫描得到的原始能力；user_data、ai_data、stats 三个映射是权威覆盖层。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Store {
    pub abilities: Vec<Ability>,
    pub user_data: BTreeMap<String, UserData>,
    pub ai_data: BTreeMap<String, AIData>,
    pub stats: BTreeMap<String, Stats>,
}

impl Store {
    pub fn merged_abilities(&self) -> Vec<Ability> {
        self.abilities
            .iter()
            .cloned()
            .map(|raw| {
                let id = raw.id.clone();
                let ai = self.ai_data.get(&id).cloned().unwrap_or_default();
                let user = self.user_data.get(&id).cloned().unwrap_or_default();
                let stats = self.stats.get(&id).cloned().unwrap_or_default();

                // 合并优先级以权威覆盖层为准，避免原始能力里残留的 ai/user/stats 与权威数据漂移。
                merge_layers_with_stats(raw, ai, user, stats)
            })
            .collect()
    }

    fn hoist_legacy_overlays(&mut self) {
        for ability in &mut self.abilities {
            let id = ability.id.clone();

            // 旧 schema 曾把覆盖层直接塞进 abilities；加载时提升到权威映射，避免之后视图合并时丢失用户数据。
            if !self.user_data.contains_key(&id) && ability.user != UserData::default() {
                self.user_data.insert(id.clone(), ability.user.clone());
            }

            if !self.ai_data.contains_key(&id) && ability.ai != AIData::default() {
                self.ai_data.insert(id.clone(), ability.ai.clone());
            }

            if !self.stats.contains_key(&id) && ability.stats != Stats::default() {
                self.stats.insert(id, ability.stats.clone());
            }

            // abilities 只保留扫描事实，覆盖层一律从权威映射读取。
            ability.user = UserData::default();
            ability.ai = AIData::default();
            ability.stats = Stats::default();
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoreLoadResult {
    pub store: Store,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatsCacheLoadResult {
    pub stats: BTreeMap<String, Stats>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum StoreError {
    PathOutsideDataDir(PathBuf),
    Io(io::Error),
    Json(serde_json::Error),
}

impl Display for StoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::PathOutsideDataDir(path) => {
                write!(formatter, "路径不在项目 data 目录内: {}", path.display())
            }
            StoreError::Io(error) => write!(formatter, "文件读写失败: {error}"),
            StoreError::Json(error) => write!(formatter, "JSON 处理失败: {error}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub fn resolve_project_data_path(
    project_dir: impl AsRef<Path>,
    relative_path: impl AsRef<Path>,
) -> Result<PathBuf, StoreError> {
    match util::resolve_project_data_path(project_dir.as_ref(), relative_path.as_ref()) {
        Ok(path) => Ok(path),
        Err(DataPathError::Outside(path)) => Err(StoreError::PathOutsideDataDir(path)),
        Err(DataPathError::Io(error)) => Err(StoreError::Io(error)),
    }
}

pub fn load_store(
    project_dir: impl AsRef<Path>,
    relative_path: impl AsRef<Path>,
) -> Result<StoreLoadResult, StoreError> {
    let path = resolve_project_data_path(project_dir, relative_path)?;

    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(StoreLoadResult {
                store: Store::default(),
                warnings: vec![format!("存储文件不存在，已使用空存储: {}", path.display())],
            });
        }
        Err(error) => return Err(StoreError::Io(error)),
    };

    match serde_json::from_str::<Store>(&content) {
        Ok(mut store) => {
            store.hoist_legacy_overlays();

            Ok(StoreLoadResult {
                store,
                warnings: Vec::new(),
            })
        }
        Err(error) => {
            // JSON 降级读取：文件损坏时返回空存储和警告，让应用可以继续启动并提示用户。
            Ok(StoreLoadResult {
                store: Store::default(),
                warnings: vec![format!("存储 JSON 无法解析，已使用空存储: {error}")],
            })
        }
    }
}

pub fn save_store(
    project_dir: impl AsRef<Path>,
    relative_path: impl AsRef<Path>,
    store: &Store,
) -> Result<(), StoreError> {
    let project_dir = project_dir.as_ref();
    let relative_path = relative_path.as_ref();
    let path = resolve_project_data_path(project_dir, relative_path)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // 创建父目录后重新校验真实父路径，防止符号链接或目录联接在创建前后变化，把存储写到项目 data 目录外。
    let path = resolve_project_data_path(project_dir, relative_path)?;
    let content = serde_json::to_string_pretty(store)?;

    write_store_atomically(&path, content.as_bytes())?;
    Ok(())
}

pub fn load_stats_cache(
    project_dir: impl AsRef<Path>,
) -> Result<BTreeMap<String, Stats>, StoreError> {
    Ok(load_stats_cache_report(project_dir)?.stats)
}

pub fn load_stats_cache_report(
    project_dir: impl AsRef<Path>,
) -> Result<StatsCacheLoadResult, StoreError> {
    let path = resolve_project_data_path(project_dir, STATS_CACHE_FILE)?;

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(stats) => Ok(StatsCacheLoadResult {
                stats,
                warnings: Vec::new(),
            }),
            Err(error) => {
                // stats-cache 是可重建缓存；坏 JSON 不能阻塞启动，只降级为空并把 warning 交给 UI/日志呈现。
                Ok(StatsCacheLoadResult {
                    stats: BTreeMap::new(),
                    warnings: vec![format!(
                        "stats-cache JSON 无法解析，已使用空缓存 {}: {error}",
                        path.display()
                    )],
                })
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(StatsCacheLoadResult {
            stats: BTreeMap::new(),
            warnings: Vec::new(),
        }),
        Err(error) => Err(StoreError::Io(error)),
    }
}

pub fn save_stats_cache(
    project_dir: impl AsRef<Path>,
    stats: &BTreeMap<String, Stats>,
) -> Result<(), StoreError> {
    let project_dir = project_dir.as_ref();
    let path = resolve_project_data_path(project_dir, STATS_CACHE_FILE)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // stats-cache 与 store 使用同一条 data 目录边界，避免缓存刷新时把项目外文件当目标覆盖。
    let path = resolve_project_data_path(project_dir, STATS_CACHE_FILE)?;
    let content = serde_json::to_string_pretty(stats)?;

    write_store_atomically(&path, content.as_bytes())?;
    Ok(())
}

fn write_store_atomically(path: &Path, content: &[u8]) -> io::Result<()> {
    let temp_path = sibling_file_path(path, "tmp");
    let _ = fs::remove_file(&temp_path);

    let write_result = (|| {
        let mut file = File::create(&temp_path)?;
        // 缓存和用户数据如果只写入半截，下次启动可能误判为坏 JSON；先写临时文件并同步，再替换目标文件。
        file.write_all(content)?;
        file.sync_all()?;
        Ok::<(), io::Error>(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    replace_with_temp_file(&temp_path, path)
}

fn replace_with_temp_file(temp_path: &Path, target_path: &Path) -> io::Result<()> {
    match fs::rename(temp_path, target_path) {
        Ok(()) => Ok(()),
        Err(first_error) if target_path.exists() => {
            if !target_path.is_file() {
                let _ = fs::remove_file(temp_path);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("存储目标不是文件: {}", target_path.display()),
                ));
            }

            let backup_path = sibling_file_path(target_path, "bak");
            let _ = fs::remove_file(&backup_path);

            fs::rename(target_path, &backup_path).map_err(|error| {
                let _ = fs::remove_file(temp_path);
                if error.kind() == io::ErrorKind::AlreadyExists {
                    first_error
                } else {
                    error
                }
            })?;

            match fs::rename(temp_path, target_path) {
                Ok(()) => {
                    let _ = fs::remove_file(backup_path);
                    Ok(())
                }
                Err(error) => {
                    let _ = fs::rename(&backup_path, target_path);
                    let _ = fs::remove_file(temp_path);
                    Err(error)
                }
            }
        }
        Err(error) => {
            let _ = fs::remove_file(temp_path);
            Err(error)
        }
    }
}

fn sibling_file_path(path: &Path, suffix: &str) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "store.json".into());
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    parent.join(format!(
        ".{file_name}.{suffix}-{}-{nonce}",
        std::process::id()
    ))
}
