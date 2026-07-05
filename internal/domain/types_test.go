package domain

import (
	"reflect"
	"testing"
	"time"
)

func TestMergeAbilityKeepsUserDataAheadOfAI(t *testing.T) {
	raw := Ability{
		ID:          "skill:diagnosing-bugs",
		Kind:        KindSkill,
		Name:        "diagnosing-bugs",
		Description: "Original English description",
	}
	user := UserData{
		Alias:    "系统排障",
		Tags:     []string{"常用", "调试"},
		Note:     "我的备注",
		Favorite: true,
		Hidden:   true,
		CustomTemplates: []CallTemplate{{
			Title: "我的模板",
			Text:  "请使用 $diagnosing-bugs 排查这个问题",
		}},
	}
	ai := AIData{
		Summary: "AI 生成摘要",
		Tags:    []string{"调试", "工程"},
		Templates: []CallTemplate{{
			Title: "AI 模板",
			Text:  "请使用 $diagnosing-bugs 帮我系统排查",
		}},
		UseScenarios:   []string{"问题复现"},
		AvoidScenarios: []string{"无需排障"},
	}
	stats := Stats{
		UsageCount:   9,
		SessionsUsed: 3,
		Mentions:     7,
		LastUsedAt:   time.Date(2026, 7, 2, 12, 0, 0, 0, time.Local),
		Known:        true,
	}

	merged := MergeAbility(raw, user, ai, stats)

	if merged.DisplayName != "系统排障" {
		t.Fatalf("DisplayName = %q, want user alias", merged.DisplayName)
	}
	if merged.Summary != "AI 生成摘要" {
		t.Fatalf("Summary = %q, want AI summary", merged.Summary)
	}
	if !merged.User.Favorite || !merged.User.Hidden || merged.User.Note != "我的备注" {
		t.Fatalf("user fields were not preserved: %+v", merged.User)
	}
	wantTags := []string{"常用", "调试", "工程"}
	if !reflect.DeepEqual(merged.Tags, wantTags) {
		t.Fatalf("Tags = %#v, want %#v", merged.Tags, wantTags)
	}
	if len(merged.CallTemplates) != 2 || merged.CallTemplates[0].Title != "我的模板" {
		t.Fatalf("CallTemplates = %#v, want user template first", merged.CallTemplates)
	}
	if merged.Stats.UsageCount != 9 || !merged.Stats.Known || merged.Stats.SessionsUsed != 3 || merged.Stats.Mentions != 7 {
		t.Fatalf("Stats = %+v, want injected stats", merged.Stats)
	}
}

func TestMergeAbilityFallsBackToRawDescriptionAndDefaultTemplate(t *testing.T) {
	raw := Ability{
		ID:          "skill:teach",
		Kind:        KindSkill,
		Name:        "teach",
		Description: "Teach a concept",
	}

	merged := MergeAbility(raw, UserData{}, AIData{}, Stats{})

	if merged.DisplayName != "teach" {
		t.Fatalf("DisplayName = %q, want raw name", merged.DisplayName)
	}
	if merged.Summary != "Teach a concept" {
		t.Fatalf("Summary = %q, want raw description", merged.Summary)
	}
	if len(merged.CallTemplates) != 1 {
		t.Fatalf("default templates = %d, want 1", len(merged.CallTemplates))
	}
	if merged.CallTemplates[0].Text != "请使用 $teach " {
		t.Fatalf("default template = %q", merged.CallTemplates[0].Text)
	}
}
