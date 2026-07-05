//go:build windows

package ui

import (
	"strings"
	"testing"

	"codex-atlas/internal/domain"
)

func TestLanguageBadge(t *testing.T) {
	tests := []struct {
		name string
		item domain.Ability
		want string
	}{
		{
			name: "中文摘要",
			item: domain.Ability{
				Kind:    domain.KindSkill,
				Summary: "用于调试和验证工作流的技能。",
			},
			want: "中文",
		},
		{
			name: "英文描述",
			item: domain.Ability{
				Kind:        domain.KindSkill,
				Description: "Use this before implementing a feature or changing behavior.",
			},
			want: "英文",
		},
		{
			name: "缓存中文优先",
			item: domain.Ability{
				Kind:        domain.KindSkill,
				Summary:     "用于实现前梳理需求和方案。",
				Description: "Use this before implementing a feature or changing behavior.",
			},
			want: "中文优先",
		},
		{
			name: "unknown",
			item: domain.Ability{
				Kind: domain.KindSkill,
				Name: "$x",
			},
			want: "未知",
		},
		{
			name: "tag hint",
			item: domain.Ability{
				Kind: domain.KindSkill,
				Tags: []string{"原生中文"},
			},
			want: "中文",
		},
		{
			name: "non skill",
			item: domain.Ability{
				Kind:        domain.KindTool,
				Description: "Use this before implementing a feature or changing behavior.",
			},
			want: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := languageBadge(tt.item); got != tt.want {
				t.Fatalf("languageBadge() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestSidebarBoundsStayInsideScreen(t *testing.T) {
	tests := []struct {
		name         string
		screenWidth  int32
		screenHeight int32
		wantWidth    int32
		wantHeight   int32
	}{
		{name: "normal screen", screenWidth: 1920, screenHeight: 1080, wantWidth: 680, wantHeight: 780},
		{name: "small screen", screenWidth: 640, screenHeight: 600},
		{name: "tiny screen", screenWidth: 500, screenHeight: 400},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			x, y, width, height := sidebarBounds(tt.screenWidth, tt.screenHeight)
			if x < 0 || y < 0 {
				t.Fatalf("sidebarBounds() origin = (%d,%d), want non-negative", x, y)
			}
			if x+width > tt.screenWidth {
				t.Fatalf("sidebarBounds() x+width = %d, screenWidth = %d", x+width, tt.screenWidth)
			}
			if y+height > tt.screenHeight {
				t.Fatalf("sidebarBounds() y+height = %d, screenHeight = %d", y+height, tt.screenHeight)
			}
			minWidth, minHeight := sidebarMinSize(width, height)
			if minWidth > width || minHeight > height {
				t.Fatalf("sidebarMinSize() = %dx%d exceeds bounds %dx%d", minWidth, minHeight, width, height)
			}
			if tt.wantWidth > 0 && width != tt.wantWidth {
				t.Fatalf("sidebarBounds() width = %d, want %d", width, tt.wantWidth)
			}
			if tt.wantHeight > 0 && height != tt.wantHeight {
				t.Fatalf("sidebarBounds() height = %d, want %d", height, tt.wantHeight)
			}
		})
	}
}

func TestClipRunes(t *testing.T) {
	if got := clipRunes("短文本", 10); got != "短文本" {
		t.Fatalf("clipRunes short = %q", got)
	}
	if got := clipRunes("这是一个很长的中文能力名称", 6); got != "这是一个很..." {
		t.Fatalf("clipRunes long = %q", got)
	}
	if got := clipRunes("abcdef", 0); got != "" {
		t.Fatalf("clipRunes zero = %q", got)
	}
}

func TestAbilityListValueClipsLongNameAndTags(t *testing.T) {
	model := &abilityListModel{items: []domain.Ability{{
		Kind:        domain.KindSkill,
		DisplayName: "这是一个特别特别特别特别特别长的能力名称",
		Description: "Use this before implementing a feature or changing behavior.",
		Stats:       domain.Stats{Known: true, UsageCount: 12},
		Tags:        []string{"非常非常非常非常长的标签", "second"},
	}}}

	got := model.Value(0).(string)
	if len([]rune(got)) > 96 {
		t.Fatalf("Value() length = %d, want <= 96; value=%q", len([]rune(got)), got)
	}
	if !contains(got, "...") || contains(got, "这是一个特别特别特别特别特别长的能力名称") {
		t.Fatalf("Value() = %q, want clipped long name", got)
	}
	if contains(got, "#非常非常非常非常长的标签") {
		t.Fatalf("Value() = %q, want clipped long tag", got)
	}
}

func contains(text, want string) bool {
	return strings.Contains(text, want)
}
