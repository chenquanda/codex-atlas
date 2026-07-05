//go:build windows

package ui

import (
	"fmt"
	"strings"

	"codex-atlas/internal/domain"

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
	tags := strings.Join(item.Tags, " / ")
	if tags != "" {
		tags = "  [" + tags + "]"
	}
	stats := ""
	if item.Kind == domain.KindSkill {
		stats = "  " + usageLabel(item.Stats)
	}
	return fmt.Sprintf("%s %s%s%s", prefix, name, tags, stats)
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
