//go:build windows

package ui

import (
	"testing"

	"codex-atlas/internal/app"
	"codex-atlas/internal/translate"
)

func TestSkillDetailShowsTranslationByDefaultOnlyWhenCached(t *testing.T) {
	doc := app.SkillDocumentView{
		SourceText:     "Use this skill before implementing.",
		DisplayText:    "实现前使用这个技能。",
		Language:       translate.LanguageEnglish,
		HasTranslation: true,
		CanTranslate:   true,
	}

	if !showTranslationByDefault(doc) {
		t.Fatal("showTranslationByDefault() = false, want true for cached translation")
	}
	if got := skillDetailDisplayText(doc, true); got != "实现前使用这个技能。" {
		t.Fatalf("skillDetailDisplayText(cached translation) = %q", got)
	}
}

func TestSkillDetailFallsBackToSourceWithoutCachedTranslation(t *testing.T) {
	doc := app.SkillDocumentView{
		SourceText:   "Use this skill before implementing.",
		DisplayText:  "Use this skill before implementing.",
		Language:     translate.LanguageEnglish,
		CanTranslate: true,
	}

	if showTranslationByDefault(doc) {
		t.Fatal("showTranslationByDefault() = true, want false without cached translation")
	}
	if got := skillDetailDisplayText(doc, true); got != "Use this skill before implementing." {
		t.Fatalf("skillDetailDisplayText(no cache) = %q", got)
	}
}

func TestSkillDetailStatusChineseNeedsNoTranslation(t *testing.T) {
	doc := app.SkillDocumentView{
		SourceText:     "这是一个中文技能说明。",
		DisplayText:    "这是一个中文技能说明。",
		Language:       translate.LanguageChinese,
		CanTranslate:   false,
		RelativePath:   "SKILL.md",
		AbsolutePath:   `C:\skills\teach\SKILL.md`,
		SkillID:        "skill:teach",
		HasTranslation: false,
	}

	if got := skillDetailStatus(doc); got != "中文，无需翻译" {
		t.Fatalf("skillDetailStatus(chinese) = %q", got)
	}
}

func TestTranslateActionVisibleOnlyForTranslatableDocuments(t *testing.T) {
	chinese := app.SkillDocumentView{
		Language:     translate.LanguageChinese,
		CanTranslate: false,
	}
	if translateActionVisible(chinese) {
		t.Fatal("translate action should be hidden for Chinese source documents")
	}

	english := app.SkillDocumentView{
		Language:     translate.LanguageEnglish,
		CanTranslate: true,
	}
	if !translateActionVisible(english) {
		t.Fatal("translate action should be visible for English source documents")
	}
}

func TestSkillFileListModelValueUsesRelativePath(t *testing.T) {
	model := &skillFileListModel{}
	model.setItems([]app.SkillFileView{
		{RelativePath: "SKILL.md", Name: "SKILL.md"},
		{RelativePath: "references/guide.md", Name: "guide.md"},
	})

	if got := model.ItemCount(); got != 2 {
		t.Fatalf("ItemCount() = %d, want 2", got)
	}
	if got := model.Value(1); got != "references/guide.md" {
		t.Fatalf("Value(1) = %q, want relative path", got)
	}
	if got := model.Value(99); got != "" {
		t.Fatalf("Value(out of range) = %q, want empty string", got)
	}
}

func TestShouldLoadAfterDefaultSelection(t *testing.T) {
	if !shouldLoadAfterDefaultSelection(0, 0) {
		t.Fatal("same index should load manually because Walk may not publish a change event")
	}
	if shouldLoadAfterDefaultSelection(0, 1) {
		t.Fatal("changed index should not load manually because CurrentIndexChanged will load it")
	}
}
