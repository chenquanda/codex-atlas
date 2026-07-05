//go:build windows

package ui

import (
	"fmt"
	"strings"

	"codex-atlas/internal/domain"
	"codex-atlas/internal/translate"

	"github.com/lxn/walk"
)

type abilityListModel struct {
	walk.ListModelBase
	items []domain.Ability
}

func (m *abilityListModel) ItemCount() int {
	return len(m.items)
}

func (m *abilityListModel) Value(index int) interface{} {
	if index < 0 || index >= len(m.items) {
		return ""
	}
	item := m.items[index]
	prefix := " "
	if item.User.Favorite {
		prefix = "*"
	}
	name := item.DisplayName
	if name == "" {
		name = item.Name
	}
	name = clipRunes(name, 18)
	details := []string{}
	if item.Kind == domain.KindSkill {
		details = append(details, usageLabel(item.Stats), languageBadge(item))
	}
	if tags := tagPreview(item.Tags, 3); tags != "" {
		details = append(details, tags)
	}
	if len(details) == 0 {
		return fmt.Sprintf("%s %s", prefix, name)
	}
	return fmt.Sprintf("%s %s    %s", prefix, name, strings.Join(nonEmptyStrings(details), "  |  "))
}

func (m *abilityListModel) setItems(items []domain.Ability) {
	m.items = items
	m.PublishItemsReset()
}

func (m *abilityListModel) current(index int) (domain.Ability, bool) {
	if index < 0 || index >= len(m.items) {
		return domain.Ability{}, false
	}
	return m.items[index], true
}

func usageLabel(stats domain.Stats) string {
	if !stats.Known {
		return "未统计"
	}
	return fmt.Sprintf("%d 使用", stats.UsageCount)
}

func usageDetailLabel(stats domain.Stats) string {
	if !stats.Known {
		return "未统计"
	}
	return fmt.Sprintf("%d 次使用", stats.UsageCount)
}

func usageSortValue(stats domain.Stats) int {
	if !stats.Known {
		return -1
	}
	return stats.UsageCount
}

func languageBadge(item domain.Ability) string {
	if item.Kind != domain.KindSkill {
		return ""
	}

	summary := strings.TrimSpace(item.Summary)
	description := strings.TrimSpace(item.Description)
	tags := strings.Join(item.Tags, " ")

	// 只用列表已持有的摘要、描述和标签做轻量启发，避免为了语言标记额外读取 skill 文件。
	if hasChineseNativeHint(tags) {
		return "中文"
	}
	summaryLanguage := translate.DetectLanguage(summary)
	descriptionLanguage := translate.DetectLanguage(description)
	if summaryLanguage == translate.LanguageChinese && descriptionLanguage == translate.LanguageEnglish && summary != description {
		return "中文优先"
	}
	if summaryLanguage == translate.LanguageChinese || descriptionLanguage == translate.LanguageChinese {
		return "中文"
	}
	if summaryLanguage == translate.LanguageEnglish || descriptionLanguage == translate.LanguageEnglish {
		return "英文"
	}
	if translate.DetectLanguage(tags) == translate.LanguageChinese {
		return "中文"
	}
	return "未知"
}

func hasChineseNativeHint(tags string) bool {
	lowered := strings.ToLower(tags)
	return strings.Contains(tags, "原生中文") ||
		strings.Contains(tags, "中文") ||
		strings.Contains(lowered, "chinese")
}

func tagPreview(tags []string, limit int) string {
	if limit <= 0 {
		return ""
	}
	var out []string
	for _, tag := range tags {
		tag = strings.TrimSpace(tag)
		if tag == "" {
			continue
		}
		out = append(out, "#"+clipRunes(tag, 8))
		if len(out) >= limit {
			break
		}
	}
	return strings.Join(out, " ")
}

func nonEmptyStrings(values []string) []string {
	out := values[:0]
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value != "" {
			out = append(out, value)
		}
	}
	return out
}

func clipRunes(text string, max int) string {
	text = strings.TrimSpace(text)
	if max <= 0 {
		return ""
	}
	runes := []rune(text)
	if len(runes) <= max {
		return text
	}
	if max == 1 {
		return "..."
	}
	// 这里只截断列表预览文本，避免超长名称/标签把单行撑得过宽；详情面板仍展示原始内容。
	return string(runes[:max-1]) + "..."
}
