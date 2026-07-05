use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter},
    sync::{RwLockReadGuard, RwLockWriteGuard},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::{
    app_state::{AppState, SharedAppState, STORE_FILE},
    domain::{Ability, UserData},
    scanner::scan_all,
    skilldoc::{
        list_skill_files as list_skill_files_for_ability,
        open_skill_detail_window as open_skill_detail_window_for_ability,
        read_skill_file as read_skill_file_for_ability, skill_detail_window_target,
        SkillDetailWindowInfo, SkillDocError, SkillFileContent, SkillFileList,
    },
    stats::{refresh_usage_stats, AbilityUsageIndex},
    store::{save_stats_cache, save_store, Store, StoreError},
};

#[tauri::command]
pub fn health() -> &'static str {
    "Codex Atlas"
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct UserDataPatch {
    #[serde(default, deserialize_with = "deserialize_nullable_patch_field")]
    pub alias: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub tags_overridden: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_nullable_patch_field")]
    pub note: Option<Option<String>>,
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_nullable_patch_field")]
    pub custom_template: Option<Option<String>>,
}

impl UserDataPatch {
    fn apply_to(self, user_data: &mut UserData) {
        if let Some(alias) = self.alias {
            user_data.alias = alias;
        }
        if let Some(tags) = self.tags {
            user_data.tags = tags;
            user_data.tags_overridden = true;
        }
        if let Some(tags_overridden) = self.tags_overridden {
            user_data.tags_overridden = tags_overridden;
        }
        if let Some(note) = self.note {
            user_data.note = note;
        }
        if let Some(favorite) = self.favorite {
            user_data.favorite = favorite;
        }
        if let Some(hidden) = self.hidden {
            user_data.hidden = hidden;
        }
        if let Some(custom_template) = self.custom_template {
            user_data.custom_template = custom_template;
        }
    }
}

fn deserialize_nullable_patch_field<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // IPC patch 语义需要区分三种状态：字段缺失(None)表示不修改；
    // 字段出现且为 null(Some(None))表示清空；字段出现且为字符串(Some(Some))表示设置新值。
    Option::<String>::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScanSummary {
    pub ability_count: usize,
    pub warning_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatsSummary {
    pub ability_count: usize,
    pub warning_count: usize,
    pub warnings: Vec<String>,
    pub complete: bool,
    pub saved_cache: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandError {
    pub message: String,
}

impl CommandError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn ability_not_found(id: &str) -> Self {
        Self::new(format!("找不到能力: {id}"))
    }
}

impl Display for CommandError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<StoreError> for CommandError {
    fn from(error: StoreError) -> Self {
        Self::new(error.to_string())
    }
}

impl From<SkillDocError> for CommandError {
    fn from(error: SkillDocError) -> Self {
        Self::new(error.to_string())
    }
}

pub type CommandResult<T> = Result<T, CommandError>;

#[tauri::command]
pub fn list_abilities(state: State<'_, SharedAppState>) -> Result<Vec<Ability>, String> {
    command_result(list_abilities_state(state.inner()))
}

#[tauri::command]
pub fn get_ability(state: State<'_, SharedAppState>, id: String) -> Result<Ability, String> {
    command_result(get_ability_state(state.inner(), &id))
}

#[tauri::command]
pub fn update_user_data(
    state: State<'_, SharedAppState>,
    id: String,
    patch: UserDataPatch,
) -> Result<Ability, String> {
    command_result(update_user_data_state(state.inner(), &id, patch))
}

#[tauri::command]
pub fn refresh_scan(state: State<'_, SharedAppState>) -> Result<ScanSummary, String> {
    command_result(refresh_scan_state(state.inner()))
}

#[tauri::command]
pub fn refresh_stats(state: State<'_, SharedAppState>) -> Result<StatsSummary, String> {
    command_result(refresh_stats_state(state.inner()))
}

#[tauri::command]
pub fn copy_call_template(state: State<'_, SharedAppState>, id: String) -> Result<String, String> {
    command_result(copy_call_template_state(state.inner(), &id))
}

#[tauri::command]
pub fn open_skill_detail_window(
    app: AppHandle,
    state: State<'_, SharedAppState>,
    id: String,
) -> Result<SkillDetailWindowInfo, String> {
    command_result(open_skill_detail_window_command(&app, state.inner(), &id))
}

#[tauri::command]
pub fn list_skill_files(
    state: State<'_, SharedAppState>,
    id: String,
) -> Result<SkillFileList, String> {
    command_result(list_skill_files_state(state.inner(), &id))
}

#[tauri::command]
pub fn read_skill_file(
    state: State<'_, SharedAppState>,
    id: String,
    relative_path: String,
) -> Result<SkillFileContent, String> {
    command_result(read_skill_file_state(state.inner(), &id, &relative_path))
}

pub fn list_abilities_state(state: &SharedAppState) -> CommandResult<Vec<Ability>> {
    let state = read_state(state)?;
    Ok(state.store.merged_abilities())
}

pub fn get_ability_state(state: &SharedAppState, id: &str) -> CommandResult<Ability> {
    let state = read_state(state)?;
    merged_ability(&state.store, id).ok_or_else(|| CommandError::ability_not_found(id))
}

pub fn update_user_data_state(
    state: &SharedAppState,
    id: &str,
    patch: UserDataPatch,
) -> CommandResult<Ability> {
    let mut state = write_state(state)?;
    ensure_ability_exists(&state.store, id)?;

    let mut next_store = state.store.clone();
    patch.apply_to(next_store.user_data.entry(id.to_string()).or_default());

    // 写锁覆盖本地磁盘提交：当前 store 小、写入是同步本地文件，用串行提交避免两个命令同时保存时出现 lost update。
    // 先保存副本，再替换内存，避免保存失败时 UI 读到未落盘的数据；后续如引入后台写入队列，再把提交协议拆出去。
    save_store(&state.project_dir, STORE_FILE, &next_store)?;
    state.store = next_store;

    merged_ability(&state.store, id).ok_or_else(|| CommandError::ability_not_found(id))
}

pub fn refresh_scan_state(state: &SharedAppState) -> CommandResult<ScanSummary> {
    let scan_roots = {
        let state = read_state(state)?;
        state.scan_roots.clone()
    };
    if scan_roots_are_empty(&scan_roots) {
        let mut state = write_state(state)?;
        let warning = "扫描根目录为空，已跳过扫描刷新以保留现有能力".to_string();
        state.warnings.push(warning.clone());
        return Ok(ScanSummary {
            ability_count: state.store.abilities.len(),
            warning_count: 1,
            warnings: vec![warning],
        });
    }

    let report = scan_all(&scan_roots);
    let warnings = report.warnings.clone();
    let ability_count = report.abilities.len();

    let mut state = write_state(state)?;
    let mut next_store = state.store.clone();

    // 写锁是扫描结果落盘的串行提交边界：扫描本身可在锁外完成，但替换 raw abilities、保存 store 和更新内存必须连续完成。
    // 扫描刷新只替换原始 abilities；用户、AI 和统计覆盖层继续留在权威映射里，不能被新扫描结果清空。
    next_store.abilities = report.abilities;
    save_store(&state.project_dir, STORE_FILE, &next_store)?;
    state.store = next_store;
    state.warnings.extend(warnings.clone());

    Ok(ScanSummary {
        ability_count,
        warning_count: warnings.len(),
        warnings,
    })
}

pub fn refresh_stats_state(state: &SharedAppState) -> CommandResult<StatsSummary> {
    let (stats_roots, abilities) = {
        let state = read_state(state)?;
        (state.stats_roots.clone(), state.store.merged_abilities())
    };
    let ability_index = AbilityUsageIndex::from_abilities(&abilities);
    let report = refresh_usage_stats(&stats_roots, &ability_index);
    let warnings = report.warnings.clone();
    let ability_count = report.stats.len();
    let complete = report.complete;
    let saved_cache = if report.can_save_cache() {
        let mut state = write_state(state)?;
        let current_ids = state
            .store
            .abilities
            .iter()
            .map(|ability| ability.id.clone())
            .collect::<BTreeSet<_>>();
        let mut next_store = state.store.clone();

        for id in current_ids {
            if let Some(stats) = report.stats.get(&id) {
                next_store.stats.insert(id, stats.clone());
            }
        }

        // 完整统计才可信：缺文件或坏 JSONL 时，0 可能只是没读到数据，不能覆盖已有统计或 unknown。
        // stats-cache 可重建，先写缓存，成功后再写权威 store；写锁保住这段同步提交，避免并发刷新互相覆盖。
        save_stats_cache(&state.project_dir, &report.stats)?;
        save_store(&state.project_dir, STORE_FILE, &next_store)?;
        state.store = next_store;
        state.warnings.extend(warnings.clone());
        true
    } else {
        let mut state = write_state(state)?;
        state.warnings.extend(warnings.clone());
        false
    };

    Ok(StatsSummary {
        ability_count,
        warning_count: warnings.len(),
        warnings,
        complete,
        saved_cache,
    })
}

pub fn copy_call_template_state(state: &SharedAppState, id: &str) -> CommandResult<String> {
    let ability = get_ability_state(state, id)?;
    Ok(ability
        .effective_call_template()
        .filter(|template| !template.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| default_call_template(&ability)))
}

fn read_state(state: &SharedAppState) -> CommandResult<RwLockReadGuard<'_, AppState>> {
    state
        .read()
        .map_err(|_| CommandError::new("应用状态读取失败，请重启 Codex Atlas 后重试"))
}

fn write_state(state: &SharedAppState) -> CommandResult<RwLockWriteGuard<'_, AppState>> {
    state
        .write()
        .map_err(|_| CommandError::new("应用状态写入失败，请重启 Codex Atlas 后重试"))
}

fn ensure_ability_exists(store: &Store, id: &str) -> CommandResult<()> {
    if store.abilities.iter().any(|ability| ability.id == id) {
        Ok(())
    } else {
        Err(CommandError::ability_not_found(id))
    }
}

fn merged_ability(store: &Store, id: &str) -> Option<Ability> {
    store
        .merged_abilities()
        .into_iter()
        .find(|ability| ability.id == id)
}

fn default_call_template(ability: &Ability) -> String {
    if let Some(skill_name) = ability.id.strip_prefix("skill:") {
        if !skill_name.trim().is_empty() {
            return format!("${skill_name}");
        }
    }

    format!("${}", ability.id)
}

pub fn open_skill_detail_window_state(
    state: &SharedAppState,
    id: &str,
) -> CommandResult<SkillDetailWindowInfo> {
    let ability = get_ability_state(state, id)?;

    Ok(open_skill_detail_window_for_ability(&ability)?)
}

fn open_skill_detail_window_command(
    app: &AppHandle,
    state: &SharedAppState,
    id: &str,
) -> CommandResult<SkillDetailWindowInfo> {
    let info = open_skill_detail_window_state(state, id)?;
    let target = skill_detail_window_target(&info.skill_id);

    // 独立窗口只加载本应用同源入口和编码后的 skill id；
    // 不能把文件路径塞进 URL，真实文件列表与内容仍必须通过受限 command 做 canonical path 校验。
    if let Some(window) = app.get_webview_window(&target.label) {
        window
            .show()
            .map_err(|error| CommandError::new(format!("显示 Skill 详情窗口失败: {error}")))?;
        window
            .set_focus()
            .map_err(|error| CommandError::new(format!("聚焦 Skill 详情窗口失败: {error}")))?;
        return Ok(info);
    }

    WebviewWindowBuilder::new(app, target.label, WebviewUrl::App(target.app_url.into()))
        .title(format!("Skill 详情 · {}", info.title))
        .inner_size(990.0, 720.0)
        .resizable(true)
        .build()
        .map_err(|error| CommandError::new(format!("创建 Skill 详情窗口失败: {error}")))?;

    Ok(info)
}

pub fn list_skill_files_state(state: &SharedAppState, id: &str) -> CommandResult<SkillFileList> {
    let ability = get_ability_state(state, id)?;
    Ok(list_skill_files_for_ability(&ability)?)
}

pub fn read_skill_file_state(
    state: &SharedAppState,
    id: &str,
    relative_path: &str,
) -> CommandResult<SkillFileContent> {
    let ability = get_ability_state(state, id)?;
    Ok(read_skill_file_for_ability(&ability, relative_path)?)
}

fn command_result<T>(result: CommandResult<T>) -> Result<T, String> {
    // Tauri IPC 的 Err 需要可序列化；这里统一收敛成面向用户的中文字符串，前端不需要理解 Rust 错误枚举。
    result.map_err(|error| error.to_string())
}

fn scan_roots_are_empty(roots: &crate::scanner::ScanRoots) -> bool {
    roots.skill_roots.is_empty()
        && roots.plugins_roots.is_empty()
        && roots.plugins_manifests.is_empty()
        && roots.tools_manifests.is_empty()
        && roots.apps_manifests.is_empty()
}

#[cfg(test)]
mod tests {
    use super::health;

    #[test]
    fn health_returns_product_name() {
        assert_eq!(health(), "Codex Atlas");
    }
}
