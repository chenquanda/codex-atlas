use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use serde_json::Value;

use crate::domain::{Ability, Stats};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatsRoots {
    pub conversation_files: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbilityUsageIndex {
    entries: Vec<AbilityUsageEntry>,
}

impl AbilityUsageIndex {
    pub fn from_abilities(abilities: &[Ability]) -> Self {
        let entries = abilities
            .iter()
            .map(AbilityUsageEntry::from_ability)
            .collect();

        Self { entries }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AbilityUsageEntry {
    id: String,
    mention_tokens: Vec<String>,
    skill_path_markers: Vec<Vec<String>>,
}

impl AbilityUsageEntry {
    fn from_ability(ability: &Ability) -> Self {
        Self {
            id: ability.id.clone(),
            mention_tokens: mention_tokens_for(ability),
            skill_path_markers: skill_path_markers_for(ability),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatsReport {
    pub stats: BTreeMap<String, Stats>,
    pub warnings: Vec<String>,
    pub complete: bool,
}

impl StatsReport {
    pub fn can_save_cache(&self) -> bool {
        self.complete
    }
}

pub fn refresh_usage_stats(roots: &StatsRoots, ability_index: &AbilityUsageIndex) -> StatsReport {
    let mut counts = ability_index
        .entries
        .iter()
        .map(|entry| (entry.id.clone(), 0_u64))
        .collect::<BTreeMap<_, _>>();
    let mut warnings = Vec::new();
    let mut complete = true;

    for path in &roots.conversation_files {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => {
                // 只要有一个对话文件读不到，本轮扫描就是不完整的；
                // 已读文件的计数仍可用于展示，但不能把缺失文件导致的 0 当成可信缓存保存。
                complete = false;
                warnings.push(format!("读取对话 JSONL 失败 {}: {error}", path.display()));
                continue;
            }
        };

        for (index, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let value = match serde_json::from_str::<Value>(line) {
                Ok(value) => value,
                Err(error) => {
                    // 坏 JSONL 行代表该会话文件没有完整参与统计；
                    // 已解析行仍保留计数，但本轮结果不能保存为完整缓存。
                    complete = false;
                    warnings.push(format!(
                        "解析对话 JSONL 行失败 {}:{}: {error}",
                        path.display(),
                        index + 1
                    ));
                    continue;
                }
            };

            let role = extract_role(&value).map(|role| role.to_ascii_lowercase());
            let text = extract_record_text(&value);

            match role.as_deref() {
                Some("user") => count_user_mentions(&text, ability_index, &mut counts),
                Some("assistant") => {
                    count_assistant_skill_reads(&value, ability_index, &mut counts)
                }
                // 这些角色经常包含系统技能清单、developer 约束、工具回显或函数结果；
                // 它们不是用户真实点名，也不是 assistant 真实读取 skill 文件，因此一律视为统计噪音。
                Some("system" | "developer" | "tool" | "function") => {}
                _ => {}
            }
        }
    }

    let stats = counts
        .into_iter()
        .map(|(id, count)| (id, Stats::counted(count)))
        .collect();

    StatsReport {
        stats,
        warnings,
        complete,
    }
}

fn count_user_mentions(
    text: &str,
    ability_index: &AbilityUsageIndex,
    counts: &mut BTreeMap<String, u64>,
) {
    for mention in explicit_mentions(text) {
        for entry in &ability_index.entries {
            if entry.mention_tokens.iter().any(|token| token == &mention) {
                // 同一会话甚至同一条消息内，重复写 `$skill` 表示重复显式点名；
                // 这里按出现次数逐个累加，不能为了“去重”而丢掉真实使用次数。
                *counts.entry(entry.id.clone()).or_default() += 1;
            }
        }
    }
}

fn count_assistant_skill_reads(
    value: &Value,
    ability_index: &AbilityUsageIndex,
    counts: &mut BTreeMap<String, u64>,
) {
    let evidence = assistant_read_evidence(value);

    for text in &evidence {
        for candidate in candidate_skill_paths(text) {
            if let Some(ability_id) = best_skill_path_match(&candidate, ability_index) {
                *counts.entry(ability_id).or_default() += 1;
            }
        }
    }
}

fn explicit_mentions(text: &str) -> Vec<String> {
    let code_free_text = strip_code_regions(text);
    let characters = code_free_text.chars().collect::<Vec<_>>();
    let mut mentions = Vec::new();
    let mut index = 0;

    while index < characters.len() {
        if characters[index] != '$' {
            index += 1;
            continue;
        }

        if index > 0 && is_mention_character(characters[index - 1]) {
            index += 1;
            continue;
        }

        let start = index + 1;
        let mut end = start;
        while end < characters.len() && is_mention_character(characters[end]) {
            end += 1;
        }

        if end > start {
            let token = characters[start..end]
                .iter()
                .collect::<String>()
                .to_ascii_lowercase()
                .trim_end_matches(['.', ':'])
                .to_string();
            if token
                .chars()
                .next()
                .map(|character| !character.is_ascii_digit())
                .unwrap_or(false)
            {
                mentions.push(token);
            }
        }

        index = end.max(index + 1);
    }

    mentions
}

fn is_mention_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':')
}

fn strip_code_regions(text: &str) -> String {
    let mut output = String::new();
    let mut in_fence = false;

    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        let line = strip_inline_code(line);
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&line);
    }

    output
}

fn strip_inline_code(line: &str) -> String {
    let characters = line.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut in_code = false;
    let mut fence_len = 0_usize;
    let mut index = 0;

    while index < characters.len() {
        if characters[index] == '`' {
            let mut run_len = 1_usize;
            while index + run_len < characters.len() && characters[index + run_len] == '`' {
                run_len += 1;
            }

            if !in_code {
                in_code = true;
                fence_len = run_len;
            } else if run_len == fence_len {
                in_code = false;
                fence_len = 0;
            }

            index += run_len;
            continue;
        }

        if !in_code {
            output.push(characters[index]);
        }
        index += 1;
    }

    output
}

fn mention_tokens_for(ability: &Ability) -> Vec<String> {
    let mut tokens = Vec::new();
    let id = ability.id.to_ascii_lowercase();

    if let Some(skill_id) = id.strip_prefix("skill:") {
        push_unique_token(&mut tokens, skill_id.to_string());
        push_unique_token(&mut tokens, normalize_id_part(&ability.name));
    } else if id.contains(':') {
        push_unique_token(&mut tokens, id);
    } else {
        push_unique_token(&mut tokens, normalize_id_part(&ability.id));
    }

    tokens
}

fn push_unique_token(tokens: &mut Vec<String>, token: String) {
    if !token.is_empty() && !tokens.contains(&token) {
        tokens.push(token);
    }
}

fn normalize_id_part(value: &str) -> String {
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

fn component_text(component: Component<'_>) -> Option<String> {
    match component {
        Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
        _ => None,
    }
}

fn path_components_from_path(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(component_text)
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn skill_path_markers_for(ability: &Ability) -> Vec<Vec<String>> {
    let Some(path) = ability.path.as_deref() else {
        return Vec::new();
    };
    let path_components = path_components_from_path(path);
    if path_components.last().map(String::as_str) != Some("skill.md") {
        return Vec::new();
    }

    let mut markers = Vec::new();
    push_unique_marker(&mut markers, path_components.clone());

    let id = ability.id.to_ascii_lowercase();
    if id.starts_with("skill:") {
        // 用户 skill 只加入带 `skills/<name>/SKILL.md` 来源前缀的 marker；
        // 最终的 `<name>/SKILL.md` 太短，会和插件同名 skill 共享后缀而造成误计。
        if let Some(suffix) = suffix_from_last_component(&path_components, "skills") {
            push_unique_marker(&mut markers, suffix);
        }
    } else if let Some((plugin, skill_id)) = id.split_once(':') {
        if let Some(suffix) = suffix_from_last_component(&path_components, "plugins") {
            push_unique_marker(&mut markers, suffix);
        }

        let skill_component = path_components
            .get(path_components.len().saturating_sub(2))
            .cloned()
            .unwrap_or_else(|| skill_id.to_string());
        push_unique_marker(
            &mut markers,
            vec![
                "plugins".to_string(),
                plugin.to_string(),
                "skills".to_string(),
                skill_component.clone(),
                "skill.md".to_string(),
            ],
        );
        push_unique_marker(
            &mut markers,
            vec![
                plugin.to_string(),
                "skills".to_string(),
                skill_component,
                "skill.md".to_string(),
            ],
        );
    }

    markers
}

fn suffix_from_last_component(components: &[String], needle: &str) -> Option<Vec<String>> {
    components
        .iter()
        .rposition(|component| component == needle)
        .map(|index| components[index..].to_vec())
}

fn push_unique_marker(markers: &mut Vec<Vec<String>>, marker: Vec<String>) {
    if marker.len() >= 3 && !markers.contains(&marker) {
        markers.push(marker);
    }
}

fn path_components_from_text(text: &str) -> Vec<String> {
    text.replace('\\', "/")
        .split('/')
        .map(|part| {
            part.trim_matches(|character: char| {
                matches!(
                    character,
                    '"' | '\'' | '`' | '[' | ']' | '(' | ')' | '{' | '}' | '<' | '>' | ',' | ';'
                )
            })
            .trim_end_matches(['.', '。'])
            .to_ascii_lowercase()
        })
        .filter(|part| !part.is_empty())
        .collect()
}

fn candidate_skill_paths(text: &str) -> Vec<Vec<String>> {
    text.split_whitespace()
        .filter(|part| part.to_ascii_lowercase().contains("skill.md"))
        .map(path_components_from_text)
        .filter(|parts| parts.last().map(String::as_str) == Some("skill.md"))
        .collect()
}

fn best_skill_path_match(
    candidate: &[String],
    ability_index: &AbilityUsageIndex,
) -> Option<String> {
    let mut best_len = 0_usize;
    let mut matches = Vec::new();

    for entry in &ability_index.entries {
        for marker in &entry.skill_path_markers {
            if path_marker_matches(candidate, marker, &entry.id) {
                match marker.len().cmp(&best_len) {
                    std::cmp::Ordering::Greater => {
                        best_len = marker.len();
                        matches.clear();
                        matches.push(entry.id.clone());
                    }
                    std::cmp::Ordering::Equal => matches.push(entry.id.clone()),
                    std::cmp::Ordering::Less => {}
                }
            }
        }
    }

    // 多个 ability 只有同等具体的 marker 时视为歧义，不写入统计；
    // 完整路径或带插件/用户来源的更长 marker 会自然胜过较短共享后缀。
    matches.sort();
    matches.dedup();
    if matches.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

fn path_marker_matches(candidate: &[String], marker: &[String], ability_id: &str) -> bool {
    if ability_id.starts_with("skill:") && marker.first().map(String::as_str) == Some("skills") {
        return candidate == marker;
    }

    candidate.len() >= marker.len() && &candidate[candidate.len() - marker.len()..] == marker
}

fn assistant_read_evidence(value: &Value) -> Vec<String> {
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();

    append_content_read_evidence(value, &mut output, &mut seen);
    append_known_structured_read_fields(value, &mut output, &mut seen);

    if let Some(message) = value.get("message") {
        append_content_read_evidence(message, &mut output, &mut seen);
        append_known_structured_read_fields(message, &mut output, &mut seen);
    }

    output
}

fn append_content_read_evidence(
    value: &Value,
    output: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    for key in ["content", "text"] {
        if let Some(child) = value.get(key) {
            append_strings_with_read_signal(child, output, seen);
        }
    }
}

fn append_known_structured_read_fields(
    value: &Value,
    output: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    for key in ["path", "input", "tool_calls"] {
        if let Some(child) = value.get(key) {
            append_structured_read_fields(child, output, seen);
        }
    }
}

fn append_structured_read_fields(
    value: &Value,
    output: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    match value {
        Value::String(text) => append_read_evidence(text, false, output, seen),
        Value::Array(items) => {
            for item in items {
                append_structured_read_fields(item, output, seen);
            }
        }
        Value::Object(object) => {
            for (key, child) in object {
                match key.as_str() {
                    "path" => append_read_texts(child, false, output, seen),
                    "cmd" | "command" => append_read_texts(child, true, output, seen),
                    "input" | "tool_calls" | "function" => {
                        append_structured_read_fields(child, output, seen)
                    }
                    "arguments" => append_arguments_read_fields(child, output, seen),
                    _ if child.is_object() || child.is_array() => {
                        append_structured_read_fields(child, output, seen)
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn append_arguments_read_fields(
    value: &Value,
    output: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    match value {
        Value::String(text) => match serde_json::from_str::<Value>(text) {
            Ok(parsed) => append_structured_read_fields(&parsed, output, seen),
            Err(_) => append_read_evidence(text, true, output, seen),
        },
        _ => append_structured_read_fields(value, output, seen),
    }
}

fn append_strings_with_read_signal(
    value: &Value,
    output: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    append_read_texts(value, true, output, seen);
}

fn append_read_texts(
    value: &Value,
    require_signal: bool,
    output: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    match value {
        Value::String(text) => append_read_evidence(text, require_signal, output, seen),
        Value::Array(items) => {
            for item in items {
                append_read_texts(item, require_signal, output, seen);
            }
        }
        Value::Object(object) => {
            for key in ["text", "content", "parts", "path", "cmd"] {
                if let Some(child) = object.get(key) {
                    append_read_texts(child, require_signal, output, seen);
                }
            }
        }
        _ => {}
    }
}

fn append_read_evidence(
    text: &str,
    require_signal: bool,
    output: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) {
    if !text.to_ascii_lowercase().contains("skill.md") {
        return;
    }
    if require_signal && !has_read_signal(text) {
        return;
    }
    if seen.insert(text.to_string()) {
        output.push(text.to_string());
    }
}

fn has_read_signal(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("读取")
        || lower.contains("get-content")
        || contains_signal_word(&lower, "read")
        || contains_signal_word(&lower, "open")
        || contains_signal_word(&lower, "cat")
}

fn contains_signal_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + word.len()..].chars().next();

        !before.map(is_signal_word_part).unwrap_or(false)
            && !after.map(is_signal_word_part).unwrap_or(false)
    })
}

fn is_signal_word_part(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-'
}

fn extract_role(value: &Value) -> Option<&str> {
    value
        .get("role")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/message/role").and_then(Value::as_str))
        .or_else(|| {
            value
                .pointer("/message/author/role")
                .and_then(Value::as_str)
        })
}

fn extract_record_text(value: &Value) -> String {
    let mut output = String::new();
    let mut seen = BTreeSet::new();

    for key in ["content", "text", "path"] {
        if let Some(child) = value.get(key) {
            append_json_text(child, &mut output, &mut seen);
        }
    }

    if let Some(message) = value.get("message") {
        for key in ["content", "text", "path"] {
            if let Some(child) = message.get(key) {
                append_json_text(child, &mut output, &mut seen);
            }
        }
    }

    output
}

fn append_json_text(value: &Value, output: &mut String, seen: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => append_text_once(text, output, seen),
        Value::Array(items) => {
            for item in items {
                append_json_text(item, output, seen);
            }
        }
        Value::Object(object) => {
            for key in ["text", "content", "parts", "path"] {
                if let Some(child) = object.get(key) {
                    append_json_text(child, output, seen);
                }
            }
        }
        _ => {}
    }
}

fn append_text_once(text: &str, output: &mut String, seen: &mut BTreeSet<String>) {
    if text.is_empty() || !seen.insert(text.to_string()) {
        return;
    }

    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(text);
}
