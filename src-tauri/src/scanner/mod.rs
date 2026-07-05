use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::domain::{Ability, AbilityKind};

const MAX_SCAN_DEPTH: usize = 8;
const MAX_SKILL_FILE_BYTES: u64 = 512 * 1024;
const MAX_MANIFEST_FILE_BYTES: u64 = 512 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScanRoots {
    pub skill_roots: Vec<PathBuf>,
    pub plugins_roots: Vec<PathBuf>,
    pub plugins_manifests: Vec<PathBuf>,
    pub tools_manifests: Vec<PathBuf>,
    pub apps_manifests: Vec<PathBuf>,
}

impl ScanRoots {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_skill_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.skill_roots.push(root.into());
        self
    }

    pub fn with_plugins_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.plugins_roots.push(root.into());
        self
    }

    pub fn with_plugins_manifest(mut self, path: impl Into<PathBuf>) -> Self {
        self.plugins_manifests.push(path.into());
        self
    }

    pub fn with_tools_manifest(mut self, path: impl Into<PathBuf>) -> Self {
        self.tools_manifests.push(path.into());
        self
    }

    pub fn with_apps_manifest(mut self, path: impl Into<PathBuf>) -> Self {
        self.apps_manifests.push(path.into());
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScanReport {
    pub abilities: Vec<Ability>,
    pub warnings: Vec<String>,
}

pub fn scan_all(roots: &ScanRoots) -> ScanReport {
    let mut report = ScanReport::default();

    scan_skills(roots, &mut report);
    scan_plugin_skills(roots, &mut report);
    scan_plugins(roots, &mut report);
    scan_tools(roots, &mut report);
    scan_apps(roots, &mut report);

    report
}

fn scan_skills(roots: &ScanRoots, report: &mut ScanReport) {
    for root in &roots.skill_roots {
        for skill_path in skill_files_below(root, &mut report.warnings) {
            if let Some(skill) = read_skill_file(&skill_path, None, &mut report.warnings) {
                push_ability(report, skill, &skill_path.display().to_string());
            }
        }
    }
}

fn scan_plugin_skills(roots: &ScanRoots, report: &mut ScanReport) {
    for root in &roots.plugins_roots {
        for plugin_root in plugin_skill_roots_below(root, &mut report.warnings) {
            for skill_path in skill_files_below(&plugin_root.skills_dir, &mut report.warnings) {
                if let Some(skill) = read_skill_file(
                    &skill_path,
                    Some(&plugin_root.plugin_name),
                    &mut report.warnings,
                ) {
                    push_ability(report, skill, &skill_path.display().to_string());
                }
            }
        }
    }
}

fn scan_plugins(roots: &ScanRoots, report: &mut ScanReport) {
    for path in &roots.plugins_manifests {
        scan_catalog_manifest(path, "plugins", "plugin", AbilityKind::Plugin, report);
    }
}

fn scan_tools(roots: &ScanRoots, report: &mut ScanReport) {
    for path in &roots.tools_manifests {
        scan_catalog_manifest(path, "tools", "tool", AbilityKind::Tool, report);
    }
}

fn scan_apps(roots: &ScanRoots, report: &mut ScanReport) {
    for path in &roots.apps_manifests {
        scan_catalog_manifest(path, "apps", "app", AbilityKind::App, report);
    }
}

fn skill_files_below(root: &Path, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    let Some(canonical_root) = real_directory_root(root, "skill 根目录", warnings) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let mut stack = vec![(canonical_root.clone(), 0_usize)];

    while let Some((directory, depth)) = stack.pop() {
        let mut entries = read_directory_entries(&directory, "skill 目录", warnings);
        entries.sort_by_key(|entry| entry.path());

        // 扫描边界：使用显式栈和最大深度，避免递归跟进异常深目录；
        // 每个目录项都先用 symlink_metadata 验真，再 canonicalize 确认仍在根目录内。
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let Some(metadata) = metadata_without_symlink(&path, "skill 目录项", warnings)
            else {
                continue;
            };

            if !metadata.is_dir() {
                continue;
            }

            let child_depth = depth + 1;
            if child_depth > MAX_SCAN_DEPTH {
                warnings.push(format!("超过最大扫描深度，跳过: {}", path.display()));
                continue;
            }

            let Some(canonical_dir) = canonical_child(&canonical_root, &path, warnings) else {
                continue;
            };
            let skill_file = canonical_dir.join("SKILL.md");

            match fs::symlink_metadata(&skill_file) {
                Ok(_) => {
                    if is_regular_skill_file(&canonical_root, &skill_file, warnings) {
                        files.push(skill_file);
                    }
                    continue;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    warnings.push(format!(
                        "读取 SKILL.md 候选文件元数据失败 {}: {error}",
                        skill_file.display()
                    ));
                    continue;
                }
            }

            stack.push((canonical_dir, child_depth));
        }
    }

    files.sort();
    files
}

fn plugin_skill_roots_below(root: &Path, warnings: &mut Vec<String>) -> Vec<PluginSkillRoot> {
    let Some(canonical_root) = real_directory_root(root, "插件根目录", warnings) else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    let mut stack = vec![(canonical_root.clone(), 0_usize)];

    while let Some((directory, depth)) = stack.pop() {
        let mut entries = read_directory_entries(&directory, "插件目录", warnings);
        entries.sort_by_key(|entry| entry.path());

        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let Some(metadata) = metadata_without_symlink(&path, "插件目录项", warnings)
            else {
                continue;
            };

            if !metadata.is_dir() {
                continue;
            }

            let child_depth = depth + 1;
            if child_depth > MAX_SCAN_DEPTH {
                warnings.push(format!(
                    "超过最大扫描深度，跳过插件目录: {}",
                    path.display()
                ));
                continue;
            }

            let Some(canonical_dir) = canonical_child(&canonical_root, &path, warnings) else {
                continue;
            };

            if canonical_dir.file_name().and_then(|name| name.to_str()) == Some("skills") {
                if let Some(plugin_name) = infer_plugin_name(&canonical_root, &canonical_dir) {
                    roots.push(PluginSkillRoot {
                        plugin_name,
                        skills_dir: canonical_dir,
                    });
                } else {
                    warnings.push(format!(
                        "无法推导插件 skill 前缀，跳过: {}",
                        canonical_dir.display()
                    ));
                }
                continue;
            }

            stack.push((canonical_dir, child_depth));
        }
    }

    roots.sort_by(|left, right| {
        left.plugin_name
            .cmp(&right.plugin_name)
            .then_with(|| left.skills_dir.cmp(&right.skills_dir))
    });
    roots.dedup_by(|left, right| left.skills_dir == right.skills_dir);
    roots
}

fn real_directory_root(root: &Path, context: &str, warnings: &mut Vec<String>) -> Option<PathBuf> {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            warnings.push(format!(
                "读取 {context} 元数据失败 {}: {error}",
                root.display()
            ));
            return None;
        }
    };

    // 根目录本身也不能是 symlink；否则 read_dir 会跟到外部目录。
    let file_type = metadata.file_type();
    if file_type.is_symlink() || !file_type.is_dir() {
        warnings.push(format!("跳过非真实目录 {context}: {}", root.display()));
        return None;
    }

    match root.canonicalize() {
        Ok(canonical) => Some(canonical),
        Err(error) => {
            warnings.push(format!(
                "解析 {context} 真实路径失败 {}: {error}",
                root.display()
            ));
            None
        }
    }
}

fn read_directory_entries(
    directory: &Path,
    context: &str,
    warnings: &mut Vec<String>,
) -> Vec<fs::DirEntry> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!(
                "无法读取 {context} {}: {error}",
                directory.display()
            ));
            return Vec::new();
        }
    };
    let mut collected = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => collected.push(entry),
            Err(error) => warnings.push(format!(
                "读取 {context} 目录项失败 {}: {error}",
                directory.display()
            )),
        }
    }
    collected
}

fn metadata_without_symlink(
    path: &Path,
    context: &str,
    warnings: &mut Vec<String>,
) -> Option<fs::Metadata> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            warnings.push(format!(
                "读取 {context} 元数据失败 {}: {error}",
                path.display()
            ));
            return None;
        }
    };

    if metadata.file_type().is_symlink() {
        warnings.push(format!("跳过 symlink {context}: {}", path.display()));
        return None;
    }

    Some(metadata)
}

fn canonical_child(
    canonical_root: &Path,
    path: &Path,
    warnings: &mut Vec<String>,
) -> Option<PathBuf> {
    let canonical = match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(error) => {
            warnings.push(format!("解析真实路径失败 {}: {error}", path.display()));
            return None;
        }
    };

    if !canonical.starts_with(canonical_root) {
        warnings.push(format!("路径越过扫描根目录，跳过: {}", canonical.display()));
        return None;
    }

    Some(canonical)
}

fn is_regular_skill_file(
    canonical_root: &Path,
    skill_file: &Path,
    warnings: &mut Vec<String>,
) -> bool {
    let Some(metadata) = metadata_without_symlink(skill_file, "SKILL.md 候选文件", warnings)
    else {
        return false;
    };
    if !metadata.is_file() {
        warnings.push(format!(
            "跳过非普通 SKILL.md 文件: {}",
            skill_file.display()
        ));
        return false;
    }
    if canonical_child(canonical_root, skill_file, warnings).is_none() {
        return false;
    }

    true
}

fn read_skill_file(
    path: &Path,
    plugin_name: Option<&str>,
    warnings: &mut Vec<String>,
) -> Option<Ability> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            warnings.push(format!(
                "读取 skill 文件元数据失败 {}: {error}",
                path.display()
            ));
            return None;
        }
    };

    // 坏文件隔离：单个 SKILL.md 的编码、大小或读取错误只记录 warning，
    // 全局扫描继续处理其他能力，避免 UI 因一个坏插件/skill 整体空白。
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        warnings.push(format!("跳过非普通 skill 文件: {}", path.display()));
        return None;
    }
    if metadata.len() > MAX_SKILL_FILE_BYTES {
        warnings.push(format!(
            "SKILL.md 超过最大文件大小，跳过: {}",
            path.display()
        ));
        return None;
    }

    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            warnings.push(format!("读取 skill 文件失败 {}: {error}", path.display()));
            return None;
        }
    };

    let directory_name = path
        .parent()
        .map(file_name)
        .unwrap_or_else(|| "skill".to_string());
    let metadata = parse_skill_metadata(&contents, &directory_name);
    let id = skill_id(plugin_name, &metadata.identity, &directory_name);

    Some(Ability::scanned(
        id,
        metadata.name,
        AbilityKind::Skill,
        Some(path.to_path_buf()),
        metadata.summary,
    ))
}

fn parse_skill_metadata(contents: &str, directory_name: &str) -> SkillMetadata {
    let (front_matter, markdown) = split_front_matter(contents);
    let name_from_front_matter = front_matter
        .as_ref()
        .and_then(|front_matter| front_matter_value(front_matter, "name"));
    let description_from_front_matter = front_matter
        .as_ref()
        .and_then(|front_matter| front_matter_value(front_matter, "description"));
    let heading = first_heading(markdown);
    let paragraph = first_paragraph(markdown);

    let identity = name_from_front_matter
        .clone()
        .unwrap_or_else(|| directory_name.to_string());
    let name = name_from_front_matter
        .or(heading)
        .unwrap_or_else(|| directory_name.to_string());
    let summary = description_from_front_matter
        .or(paragraph)
        .unwrap_or_default();

    SkillMetadata {
        identity,
        name,
        summary,
    }
}

fn split_front_matter(contents: &str) -> (Option<String>, &str) {
    let normalized = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let Some(rest) = normalized.strip_prefix("---") else {
        return (None, normalized);
    };
    let Some(rest) = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))
    else {
        return (None, normalized);
    };

    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        offset += line.len();
        if trimmed == "---" {
            let front_matter_len = offset - line.len();
            return (Some(rest[..front_matter_len].to_string()), &rest[offset..]);
        }
    }

    (None, normalized)
}

fn front_matter_value(front_matter: &str, key: &str) -> Option<String> {
    let lines = front_matter.lines().collect::<Vec<_>>();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let Some((line_key, value)) = line.split_once(':') else {
            index += 1;
            continue;
        };
        if line_key.trim() != key {
            index += 1;
            continue;
        }

        let value = clean_scalar_value(value);
        if let Some(literal) = block_scalar_literal_mode(&value) {
            return front_matter_block_value(&lines, index + 1, literal);
        }
        if !value.is_empty() {
            return Some(value);
        }

        return None;
    }

    None
}

fn front_matter_block_value(lines: &[&str], start: usize, literal: bool) -> Option<String> {
    let mut block_lines = Vec::new();
    for line in &lines[start..] {
        if line.trim().is_empty() {
            if literal {
                block_lines.push(String::new());
            }
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        block_lines.push(line.trim().to_string());
    }

    let value = if literal {
        block_lines.join("\n")
    } else {
        block_lines
            .into_iter()
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn clean_scalar_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn block_scalar_literal_mode(value: &str) -> Option<bool> {
    match value {
        "|" | "|-" | "|+" => Some(true),
        ">" | ">-" | ">+" => Some(false),
        _ => None,
    }
}

fn first_heading(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            return None;
        }

        let title = trimmed.trim_start_matches('#').trim();
        if title.is_empty() {
            None
        } else {
            Some(title.to_string())
        }
    })
}

fn first_paragraph(markdown: &str) -> Option<String> {
    let mut lines = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !lines.is_empty() {
                break;
            }
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }

        lines.push(trimmed);
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" "))
    }
}

fn scan_catalog_manifest(
    path: &Path,
    list_key: &str,
    prefix: &str,
    kind: AbilityKind,
    report: &mut ScanReport,
) {
    let Some(root) = read_manifest_value(path, list_key, &mut report.warnings) else {
        return;
    };
    let Some(entries) = root.get(list_key).and_then(Value::as_array) else {
        return;
    };

    for (index, entry) in entries.iter().enumerate() {
        let context = format!("{}[{index}] {}", list_key, path.display());
        let Some(entry) = catalog_entry_from_value(entry, &context, &mut report.warnings) else {
            continue;
        };
        push_ability(
            report,
            catalog_ability(prefix, kind.clone(), &entry, path.to_path_buf()),
            &context,
        );
    }
}

fn read_manifest_value(path: &Path, context: &str, warnings: &mut Vec<String>) -> Option<Value> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            warnings.push(format!(
                "读取 {context} manifest 元数据失败 {}: {error}",
                path.display()
            ));
            return None;
        }
    };

    // manifest 是显式配置的扫描输入，也必须先验真：拒绝 symlink、目录和其他非普通文件，
    // 避免通过 manifest 路径越界读取外部目录或用户未授权的缓存内容。
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        warnings.push(format!(
            "跳过非普通 {context} manifest 文件: {}",
            path.display()
        ));
        return None;
    }
    if metadata.len() > MAX_MANIFEST_FILE_BYTES {
        warnings.push(format!(
            "{context} manifest 超过最大文件大小，跳过: {}",
            path.display()
        ));
        return None;
    }

    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            warnings.push(format!(
                "读取 {context} manifest 失败 {}: {error}",
                path.display()
            ));
            return None;
        }
    };

    match serde_json::from_str(&contents) {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(format!(
                "解析 {context} manifest 失败 {}: {error}",
                path.display()
            ));
            None
        }
    }
}

fn catalog_entry_from_value(
    value: &Value,
    context: &str,
    warnings: &mut Vec<String>,
) -> Option<CatalogEntry> {
    let Some(name) = value.get("name").and_then(Value::as_str) else {
        warnings.push(format!("manifest entry 缺少 name，跳过: {context}"));
        return None;
    };
    if name.trim().is_empty() {
        warnings.push(format!("manifest entry 缺少 name，跳过: {context}"));
        return None;
    }

    Some(CatalogEntry {
        name: name.to_string(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn catalog_ability(
    prefix: &str,
    kind: AbilityKind,
    entry: &CatalogEntry,
    path: PathBuf,
) -> Ability {
    Ability::scanned(
        catalog_id(prefix, &entry.name),
        entry.name.clone(),
        kind,
        Some(path),
        entry.description.clone(),
    )
}

fn push_ability(report: &mut ScanReport, ability: Ability, context: &str) {
    if report
        .abilities
        .iter()
        .any(|existing| existing.id == ability.id)
    {
        report.warnings.push(format!(
            "重复 ability id {}，跳过后续来源: {context}",
            ability.id
        ));
        return;
    }

    report.abilities.push(ability);
}

fn skill_id(plugin_name: Option<&str>, skill_name: &str, fallback: &str) -> String {
    let skill = normalize_id_part(skill_name, fallback);
    match plugin_name {
        // 插件 skill 前缀集中规范化，确保 versioned cache 不会把版本号误当插件名。
        Some(plugin_name) => format!("{}:{skill}", normalize_id_part(plugin_name, "plugin")),
        None => format!("skill:{skill}"),
    }
}

fn catalog_id(kind: &str, name: &str) -> String {
    format!("{kind}:{}", normalize_id_part(name, "entry"))
}

fn normalize_id_part(value: &str, fallback: &str) -> String {
    let normalized = normalize_raw_id_part(value);
    if !normalized.is_empty() {
        return normalized;
    }

    let fallback = normalize_raw_id_part(fallback);
    if fallback.is_empty() {
        "unknown".to_string()
    } else {
        fallback
    }
}

fn normalize_raw_id_part(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_dash = false;

    for character in value.trim().to_lowercase().chars() {
        let allowed = character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '_' | '.' | '-');
        if allowed {
            output.push(character);
            last_was_dash = character == '-';
        } else if !last_was_dash {
            output.push('-');
            last_was_dash = true;
        }
    }

    output.trim_matches('-').to_string()
}

fn infer_plugin_name(canonical_root: &Path, skills_dir: &Path) -> Option<String> {
    let relative = skills_dir.strip_prefix(canonical_root).ok()?;
    let components = relative
        .components()
        .filter_map(component_to_string)
        .collect::<Vec<_>>();
    if components.last().map(String::as_str) != Some("skills") {
        return None;
    }

    match components.as_slice() {
        // direct: <root>/<plugin>/skills/<skill>/SKILL.md
        [plugin, skills] if skills == "skills" => Some(plugin.clone()),
        // cache simple: <root>/cache/<plugin>/skills/<skill>/SKILL.md
        [cache, plugin, skills] if cache == "cache" && skills == "skills" => Some(plugin.clone()),
        // versioned cache: <root>/cache/<publisher>/<plugin>/<version>/skills/<skill>/SKILL.md
        components if components.len() >= 5 && components[0] == "cache" => {
            components.get(components.len() - 3).cloned()
        }
        components if components.len() >= 2 => components.get(components.len() - 2).cloned(),
        _ => None,
    }
}

fn component_to_string(component: Component<'_>) -> Option<String> {
    match component {
        Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
        _ => None,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

struct PluginSkillRoot {
    plugin_name: String,
    skills_dir: PathBuf,
}

struct SkillMetadata {
    identity: String,
    name: String,
    summary: String,
}

struct CatalogEntry {
    name: String,
    description: String,
}
