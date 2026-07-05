use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AbilityKind {
    Skill,
    Plugin,
    Tool,
    App,
}

impl Default for AbilityKind {
    fn default() -> Self {
        Self::Skill
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Ability {
    pub id: String,
    pub name: String,
    pub kind: AbilityKind,
    pub path: Option<PathBuf>,
    pub summary: String,
    pub raw_tags: Vec<String>,
    pub ai: AIData,
    pub user: UserData,
    pub stats: Stats,
}

impl Ability {
    pub(crate) fn scanned(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: AbilityKind,
        path: Option<PathBuf>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            path,
            summary: summary.into(),
            ..Self::default()
        }
    }

    pub fn display_tags(&self) -> Vec<String> {
        self.effective_tags()
    }

    pub fn effective_tags(&self) -> Vec<String> {
        // 用户数据优先级：非空标签表示用户已设置；tags_overridden 表示用户明确保存过标签状态，即使清空也不能回退到 AI 或原始标签。
        if self.user.tags_overridden || !self.user.tags.is_empty() {
            return self.user.tags.clone();
        }

        if !self.ai.tags.is_empty() {
            return self.ai.tags.clone();
        }

        self.raw_tags.clone()
    }

    pub fn effective_call_template(&self) -> Option<&str> {
        // 用户自定义模板代表人工确认过的调用方式，不能被 AI 生成模板覆盖。
        self.user
            .custom_template
            .as_deref()
            .or(self.ai.call_template.as_deref())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Stats {
    pub usage_count: Option<u64>,
    pub last_used_at: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct UserData {
    pub alias: Option<String>,
    pub tags: Vec<String>,
    pub tags_overridden: bool,
    pub note: Option<String>,
    pub favorite: bool,
    pub hidden: bool,
    pub custom_template: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AIData {
    pub summary_zh: Option<String>,
    pub tags: Vec<String>,
    pub call_template: Option<String>,
    pub scenarios: Vec<String>,
}

pub fn merge_layers(raw: Ability, ai: AIData, user: UserData) -> Ability {
    merge_layers_with_stats(raw, ai, user, Stats::default())
}

pub fn merge_layers_with_stats(
    mut raw: Ability,
    ai: AIData,
    user: UserData,
    stats: Stats,
) -> Ability {
    // 原始能力只提供扫描事实；AI、用户数据和统计数据按覆盖层合入视图，优先级由读取方法决定。
    raw.ai = ai;
    raw.user = user;
    raw.stats = stats;
    raw
}

pub fn format_usage(usage_count: Option<u64>) -> String {
    match usage_count {
        Some(count) => format!("{count} 使用"),
        None => "未统计".to_string(),
    }
}
