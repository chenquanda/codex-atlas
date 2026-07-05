package domain

import (
	"strings"
	"time"
)

// AbilityKind 标记能力来源类别，UI 依赖它做视图切换和过滤。
type AbilityKind string

const (
	KindSkill  AbilityKind = "skill"
	KindPlugin AbilityKind = "plugin"
	KindTool   AbilityKind = "tool"
	KindApp    AbilityKind = "app"
)

// CallTemplate 是可复制回 Codex 对话里的调用话术。
type CallTemplate struct {
	Title  string `json:"title"`
	Text   string `json:"text"`
	Pinned bool   `json:"pinned,omitempty"`
}

// Stats 保存只读历史使用统计。UsageCount 是 UI 主指标，Known 表示缓存是否来自新版统计。
type Stats struct {
	UsageCount   int       `json:"usageCount"`
	SessionsUsed int       `json:"sessionsUsed"`
	Mentions     int       `json:"mentions"`
	LastUsedAt   time.Time `json:"lastUsedAt,omitempty"`
	Known        bool      `json:"-"`
}

// UserData 是用户手动整理层。扫描和 AI 整理都不能覆盖这些字段。
type UserData struct {
	Alias           string         `json:"alias,omitempty"`
	Tags            []string       `json:"tags,omitempty"`
	Note            string         `json:"note,omitempty"`
	Favorite        bool           `json:"favorite,omitempty"`
	Hidden          bool           `json:"hidden,omitempty"`
	CustomTemplates []CallTemplate `json:"customTemplates,omitempty"`
	SortWeight      int            `json:"sortWeight,omitempty"`
	UpdatedAt       time.Time      `json:"updatedAt,omitempty"`
}

// AIData 是 AI 离线整理层。重新整理时只替换这一层，不能碰 UserData。
type AIData struct {
	Summary        string         `json:"summary,omitempty"`
	Tags           []string       `json:"tags,omitempty"`
	Templates      []CallTemplate `json:"templates,omitempty"`
	UseScenarios   []string       `json:"useScenarios,omitempty"`
	AvoidScenarios []string       `json:"avoidScenarios,omitempty"`
	Similar        []string       `json:"similar,omitempty"`
	UpdatedAt      time.Time      `json:"updatedAt,omitempty"`
}

// Ability 是 UI 展示使用的合并后能力对象。
type Ability struct {
	ID             string         `json:"id"`
	Kind           AbilityKind    `json:"kind"`
	Name           string         `json:"name"`
	DisplayName    string         `json:"displayName,omitempty"`
	Plugin         string         `json:"plugin,omitempty"`
	Version        string         `json:"version,omitempty"`
	Description    string         `json:"description,omitempty"`
	Summary        string         `json:"summary,omitempty"`
	Tags           []string       `json:"tags,omitempty"`
	CallTemplates  []CallTemplate `json:"callTemplates,omitempty"`
	UseScenarios   []string       `json:"useScenarios,omitempty"`
	AvoidScenarios []string       `json:"avoidScenarios,omitempty"`
	Source         string         `json:"source,omitempty"`
	SourcePath     string         `json:"sourcePath,omitempty"`
	Directory      string         `json:"directory,omitempty"`
	Status         string         `json:"status,omitempty"`
	ChildIDs       []string       `json:"childIds,omitempty"`
	Stats          Stats          `json:"stats"`
	User           UserData       `json:"user"`
	AI             AIData         `json:"ai"`
}

// Store 是项目内 JSON 持久化结构，分开保存用户层和 AI 层。
type Store struct {
	Version  int                 `json:"version"`
	Users    map[string]UserData `json:"users"`
	AI       map[string]AIData   `json:"ai"`
	Settings map[string]string   `json:"settings"`
}

// NewStore 返回带初始化 map 的空存储，避免调用方反复判空。
func NewStore() Store {
	return Store{
		Version:  1,
		Users:    map[string]UserData{},
		AI:       map[string]AIData{},
		Settings: map[string]string{},
	}
}

// MergeAbility 按“原始扫描数据 < AI 数据 < 用户数据”的展示规则合成最终能力。
func MergeAbility(raw Ability, user UserData, ai AIData, stats Stats) Ability {
	merged := raw
	merged.User = user
	merged.AI = ai
	merged.Stats = stats

	if strings.TrimSpace(user.Alias) != "" {
		merged.DisplayName = strings.TrimSpace(user.Alias)
	} else if strings.TrimSpace(raw.DisplayName) != "" {
		merged.DisplayName = strings.TrimSpace(raw.DisplayName)
	} else {
		merged.DisplayName = raw.Name
	}

	if strings.TrimSpace(ai.Summary) != "" {
		merged.Summary = strings.TrimSpace(ai.Summary)
	} else if strings.TrimSpace(raw.Summary) != "" {
		merged.Summary = strings.TrimSpace(raw.Summary)
	} else {
		merged.Summary = strings.TrimSpace(raw.Description)
	}

	merged.Tags = mergeStrings(user.Tags, ai.Tags, raw.Tags)
	merged.CallTemplates = mergeTemplates(user.CustomTemplates, ai.Templates)
	if len(merged.CallTemplates) == 0 {
		merged.CallTemplates = mergeTemplates(defaultTemplates(raw))
	}
	merged.UseScenarios = append([]string{}, ai.UseScenarios...)
	merged.AvoidScenarios = append([]string{}, ai.AvoidScenarios...)
	return merged
}

func mergeStrings(groups ...[]string) []string {
	seen := map[string]bool{}
	var out []string
	for _, group := range groups {
		for _, item := range group {
			item = strings.TrimSpace(item)
			if item == "" || seen[item] {
				continue
			}
			seen[item] = true
			out = append(out, item)
		}
	}
	return out
}

func mergeTemplates(groups ...[]CallTemplate) []CallTemplate {
	seen := map[string]bool{}
	var out []CallTemplate
	for _, group := range groups {
		for _, item := range group {
			item.Title = strings.TrimSpace(item.Title)
			key := strings.TrimSpace(item.Text)
			if key == "" || seen[key] {
				continue
			}
			seen[key] = true
			out = append(out, item)
		}
	}
	return out
}

func defaultTemplates(raw Ability) []CallTemplate {
	if raw.Kind != KindSkill || strings.TrimSpace(raw.Name) == "" {
		return nil
	}
	return []CallTemplate{{
		Title: "默认调用",
		Text:  "请使用 $" + raw.Name + " ",
	}}
}
