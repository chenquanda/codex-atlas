use std::{
    fmt::{Display, Formatter},
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::domain::{Ability, AbilityKind};

const MAX_DEPTH: usize = 4;
const MAX_FILES: usize = 200;
const MAX_FILE_BYTES: u64 = 512 * 1024;
const BINARY_SAMPLE_BYTES: usize = 8192;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillDetailWindowInfo {
    pub skill_id: String,
    pub title: String,
    pub root_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillFileList {
    pub skill_id: String,
    pub root_path: PathBuf,
    pub files: Vec<SkillFileEntry>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillFileEntry {
    pub relative_path: String,
    pub size_bytes: u64,
    pub extension: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillFileContent {
    pub skill_id: String,
    pub relative_path: String,
    pub content: String,
    pub size_bytes: u64,
    pub language: SkillFileLanguage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillDetailWindowTarget {
    pub label: String,
    pub app_url: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SkillFileLanguage {
    Chinese,
    Other,
}

#[derive(Debug)]
pub enum SkillDocError {
    NotSkill(String),
    MissingPath(String),
    InvalidAbilityPath(String),
    InvalidRelativePath(String),
    AbsoluteRelativePath(String),
    PathEscapedRoot(PathBuf),
    Io(io::Error),
}

impl Display for SkillDocError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillDocError::NotSkill(id) => write!(formatter, "能力 {id} 不是 Skill，无法打开详情"),
            SkillDocError::MissingPath(id) => write!(formatter, "Skill {id} 缺少文件路径"),
            SkillDocError::InvalidAbilityPath(message) => formatter.write_str(message),
            SkillDocError::InvalidRelativePath(path) => {
                write!(formatter, "Skill 文件相对路径无效或包含上级目录: {path}")
            }
            SkillDocError::AbsoluteRelativePath(path) => {
                write!(
                    formatter,
                    "Skill 文件路径必须是相对路径，不能使用绝对路径: {path}"
                )
            }
            SkillDocError::PathEscapedRoot(path) => {
                write!(
                    formatter,
                    "读取路径越过 Skill 目录，已拒绝: {}",
                    path.display()
                )
            }
            SkillDocError::Io(error) => write!(formatter, "Skill 文件读取失败: {error}"),
        }
    }
}

impl std::error::Error for SkillDocError {}

impl From<io::Error> for SkillDocError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn open_skill_detail_window(ability: &Ability) -> Result<SkillDetailWindowInfo, SkillDocError> {
    let root = resolve_skill_root(ability)?;

    Ok(SkillDetailWindowInfo {
        skill_id: ability.id.clone(),
        title: ability.name.clone(),
        root_path: root,
    })
}

pub fn skill_detail_window_target(skill_id: &str) -> SkillDetailWindowTarget {
    let safe_id = sanitize_window_label_part(skill_id);
    let id_hash = stable_label_hash(skill_id);

    SkillDetailWindowTarget {
        // 窗口 label 只能使用安全字符；末尾追加原始 id 的稳定哈希，避免 `a/b` 和 `a b` 清洗后碰撞。
        label: format!("skill-detail-{safe_id}-{id_hash:016x}"),
        app_url: format!(
            "index.html?skillDetail={}",
            percent_encode_query_value(skill_id)
        ),
    }
}

pub fn list_skill_files(ability: &Ability) -> Result<SkillFileList, SkillDocError> {
    let root = resolve_skill_root(ability)?;
    let mut collector = SkillFileCollector::new(&root);
    collector.collect_directory(&root, 0);

    collector.files.sort_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.size_bytes.cmp(&right.size_bytes))
    });

    Ok(SkillFileList {
        skill_id: ability.id.clone(),
        root_path: root.clone(),
        files: collector.files,
        warnings: collector.warnings,
    })
}

pub fn read_skill_file(
    ability: &Ability,
    relative_path: &str,
) -> Result<SkillFileContent, SkillDocError> {
    let root = resolve_skill_root(ability)?;
    let normalized = normalize_relative_path(relative_path)?;
    let target = root.join(&normalized);

    // 读取边界：先 canonicalize root 和目标，再要求目标真实路径仍以 root 开头。
    // 这一步覆盖 `..`、目录联接和 symlink 逃逸；通过字符串清洗但真实路径出界的文件一律拒绝。
    let canonical_root = root.canonicalize()?;
    let canonical_target = target.canonicalize().map_err(SkillDocError::Io)?;
    if !canonical_target.starts_with(&canonical_root) {
        return Err(SkillDocError::PathEscapedRoot(canonical_target));
    }

    let metadata = fs::metadata(&canonical_target)?;
    if !metadata.is_file() {
        return Err(SkillDocError::InvalidAbilityPath(format!(
            "Skill 文件路径不是普通文件: {}",
            canonical_target.display()
        )));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(SkillDocError::InvalidAbilityPath(format!(
            "Skill 文件超过最大文件大小 {MAX_FILE_BYTES} 字节: {}",
            canonical_target.display()
        )));
    }
    if !has_readable_extension(&canonical_target) {
        return Err(SkillDocError::InvalidAbilityPath(format!(
            "Skill 文件类型不可预览: {}",
            canonical_target.display()
        )));
    }

    let bytes = fs::read(&canonical_target)?;
    let content = decode_text_file(&bytes).map_err(|message| {
        SkillDocError::InvalidAbilityPath(format!("{message}: {}", canonical_target.display()))
    })?;
    let relative_path = path_to_slash_string(&normalized);
    let language = if contains_chinese(&content) {
        SkillFileLanguage::Chinese
    } else {
        SkillFileLanguage::Other
    };

    Ok(SkillFileContent {
        skill_id: ability.id.clone(),
        relative_path,
        content,
        size_bytes: metadata.len(),
        language,
    })
}

fn resolve_skill_root(ability: &Ability) -> Result<PathBuf, SkillDocError> {
    if ability.kind != AbilityKind::Skill {
        return Err(SkillDocError::NotSkill(ability.id.clone()));
    }

    let path = ability
        .path
        .as_ref()
        .ok_or_else(|| SkillDocError::MissingPath(ability.id.clone()))?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        SkillDocError::InvalidAbilityPath(format!(
            "读取 Skill 路径元数据失败 {}: {error}",
            path.display()
        ))
    })?;

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SkillDocError::InvalidAbilityPath(format!(
            "Skill 路径必须指向真实可读文件: {}",
            path.display()
        )));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(SkillDocError::InvalidAbilityPath(format!(
            "Skill 入口文件超过最大文件大小 {MAX_FILE_BYTES} 字节: {}",
            path.display()
        )));
    }
    if !has_readable_extension(path) || is_binary_looking_file(path) {
        return Err(SkillDocError::InvalidAbilityPath(format!(
            "Skill 入口文件不是可阅读文本: {}",
            path.display()
        )));
    }

    let canonical_file = path.canonicalize().map_err(|error| {
        SkillDocError::InvalidAbilityPath(format!(
            "解析 Skill 入口真实路径失败 {}: {error}",
            path.display()
        ))
    })?;
    let root = if is_skill_markdown(&canonical_file) {
        canonical_file.parent().map(Path::to_path_buf)
    } else {
        find_skill_root_from_file(&canonical_file)
    }
    .ok_or_else(|| {
        SkillDocError::InvalidAbilityPath(format!(
            "无法从 Skill 入口推导包含 SKILL.md 的目录: {}",
            canonical_file.display()
        ))
    })?;

    // Skill 根目录以 canonical SKILL.md 所在目录为准；后续列目录和读文件都只接受这个真实目录前缀。
    root.canonicalize().map_err(|error| {
        SkillDocError::InvalidAbilityPath(format!(
            "解析 Skill 根目录真实路径失败 {}: {error}",
            root.display()
        ))
    })
}

fn find_skill_root_from_file(file: &Path) -> Option<PathBuf> {
    let mut current = file.parent();
    let mut depth = 0_usize;

    while let Some(directory) = current {
        if depth > MAX_DEPTH {
            return None;
        }
        let skill_file = directory.join("SKILL.md");
        if is_real_readable_file(&skill_file) {
            return Some(directory.to_path_buf());
        }

        current = directory.parent();
        depth += 1;
    }

    None
}

fn normalize_relative_path(relative_path: &str) -> Result<PathBuf, SkillDocError> {
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(SkillDocError::AbsoluteRelativePath(
            relative_path.to_string(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SkillDocError::InvalidRelativePath(
                    relative_path.to_string(),
                ));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(SkillDocError::InvalidRelativePath(
            relative_path.to_string(),
        ));
    }

    Ok(normalized)
}

struct SkillFileCollector<'a> {
    root: &'a Path,
    files: Vec<SkillFileEntry>,
    warnings: Vec<String>,
    hit_file_limit: bool,
}

impl<'a> SkillFileCollector<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            root,
            files: Vec::new(),
            warnings: Vec::new(),
            hit_file_limit: false,
        }
    }

    fn collect_directory(&mut self, directory: &Path, depth: usize) {
        if self.hit_file_limit {
            return;
        }

        let mut entries = match fs::read_dir(directory) {
            Ok(entries) => entries
                .filter_map(|entry| match entry {
                    Ok(entry) => Some(entry),
                    Err(error) => {
                        self.warnings.push(format!(
                            "读取 Skill 目录项失败 {}: {error}",
                            directory.display()
                        ));
                        None
                    }
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                self.warnings.push(format!(
                    "无法读取 Skill 目录 {}: {error}",
                    directory.display()
                ));
                return;
            }
        };
        entries.sort_by_key(|entry| entry.path());

        // 文件枚举策略：坏文件、二进制、依赖目录和构建目录只跳过并记录 warning；
        // 详情窗口仍尽量展示同一 Skill 内其他可读文件，避免一个坏资产拖垮整个阅读器。
        for entry in entries {
            if self.hit_file_limit {
                return;
            }

            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    self.warnings.push(format!(
                        "读取 Skill 文件元数据失败 {}: {error}",
                        path.display()
                    ));
                    continue;
                }
            };

            let canonical = match path.canonicalize() {
                Ok(canonical) => canonical,
                Err(error) => {
                    self.warnings.push(format!(
                        "解析 Skill 文件真实路径失败 {}: {error}",
                        path.display()
                    ));
                    continue;
                }
            };
            if !canonical.starts_with(self.root) {
                self.warnings.push(format!(
                    "Skill 文件越过根目录，跳过: {}",
                    canonical.display()
                ));
                continue;
            }

            if metadata.is_dir() {
                if should_skip_directory(&path) {
                    continue;
                }
                if depth + 1 > MAX_DEPTH {
                    self.warnings.push(format!(
                        "Skill 文件夹超过最大深度，跳过: {}",
                        path.display()
                    ));
                    continue;
                }
                self.collect_directory(&canonical, depth + 1);
                continue;
            }

            if !metadata.is_file() {
                self.warnings
                    .push(format!("跳过非普通 Skill 文件: {}", path.display()));
                continue;
            }
            if metadata.len() > MAX_FILE_BYTES {
                self.warnings.push(format!(
                    "Skill 文件超过最大文件大小，跳过: {}",
                    path.display()
                ));
                continue;
            }
            if !has_readable_extension(&path) || is_binary_looking_file(&canonical) {
                continue;
            }

            let Some(relative_path) = canonical
                .strip_prefix(self.root)
                .ok()
                .map(path_to_slash_string)
            else {
                self.warnings.push(format!(
                    "Skill 文件越过根目录，跳过: {}",
                    canonical.display()
                ));
                continue;
            };

            self.files.push(SkillFileEntry {
                relative_path,
                size_bytes: metadata.len(),
                extension: normalized_extension(&path),
            });

            if self.files.len() >= MAX_FILES {
                self.hit_file_limit = true;
                self.warnings.push(format!(
                    "Skill 可读文件达到数量上限 {MAX_FILES}，已停止继续列出"
                ));
            }
        }
    }
}

fn should_skip_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "node_modules"
            | "target"
            | ".git"
            | ".cache"
            | "dist"
            | "build"
            | ".next"
            | ".turbo"
            | "vendor"
            | "__pycache__"
    )
}

fn has_readable_extension(path: &Path) -> bool {
    if is_known_readable_name(path) {
        return true;
    }

    let Some(extension) = normalized_extension(path) else {
        return false;
    };

    matches!(
        extension.as_str(),
        "md" | "markdown"
            | "mdx"
            | "txt"
            | "json"
            | "jsonl"
            | "yml"
            | "yaml"
            | "toml"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "css"
            | "scss"
            | "sass"
            | "html"
            | "htm"
            | "ps1"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "py"
            | "rb"
            | "go"
            | "java"
            | "kt"
            | "kts"
            | "cs"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "sql"
            | "xml"
            | "ini"
            | "conf"
            | "cfg"
            | "env"
            | "lock"
            | "lua"
            | "r"
            | "swift"
            | "php"
            | "vue"
            | "svelte"
    )
}

fn is_known_readable_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "skill.md" | "readme" | "license" | "dockerfile" | ".gitignore" | ".env"
    )
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

fn is_binary_looking_file(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return true,
    };
    // 文件跳过策略：二进制判断只取固定前缀样本，避免为预览列表把大文本完整读入内存。
    let mut sample = vec![0_u8; BINARY_SAMPLE_BYTES];
    let sample_len = match file.read(&mut sample) {
        Ok(sample_len) => sample_len,
        Err(_) => return true,
    };
    let sample = &sample[..sample_len];

    sample.contains(&0) || std::str::from_utf8(sample).is_err()
}

fn decode_text_file(bytes: &[u8]) -> Result<String, &'static str> {
    if bytes.contains(&0) {
        return Err("Skill 文件看起来是二进制内容");
    }

    std::str::from_utf8(bytes)
        .map(|content| content.trim_start_matches('\u{feff}').to_string())
        .map_err(|_| "Skill 文件不是 UTF-8 文本")
}

fn is_real_readable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return false;
    }

    has_readable_extension(path) && !is_binary_looking_file(path)
}

fn is_skill_markdown(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case("SKILL.md"))
        .unwrap_or(false)
}

fn path_to_slash_string(path: impl AsRef<Path>) -> String {
    path.as_ref()
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn contains_chinese(content: &str) -> bool {
    content
        .chars()
        .any(|character| matches!(character, '\u{4e00}'..='\u{9fff}'))
}

fn sanitize_window_label_part(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_dash = false;

    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
            last_was_dash = false;
        } else if !last_was_dash {
            output.push('-');
            last_was_dash = true;
        }
    }

    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

fn stable_label_hash(value: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;

    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    hash
}

fn percent_encode_query_value(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.as_bytes() {
        let character = *byte as char;
        let unreserved =
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '~');
        if unreserved {
            encoded.push(character);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}
