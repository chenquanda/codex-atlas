package app

import (
	"context"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"codex-atlas/internal/ai"
	"codex-atlas/internal/domain"
	"codex-atlas/internal/scanner"
	"codex-atlas/internal/translate"
)

func TestReadSkillFileShowsEnglishSourceWhenCacheMiss(t *testing.T) {
	manager := newLoadedSkillManager(t, "teach", "SKILL.md", "# Teach\n\nThis skill teaches concepts with patient examples.", nil)

	view, err := manager.ReadSkillFile("skill:teach", "SKILL.md")
	if err != nil {
		t.Fatalf("ReadSkillFile returned error: %v", err)
	}
	if view.Language != translate.LanguageEnglish {
		t.Fatalf("language = %s, want English", view.Language)
	}
	if view.DisplayText != view.SourceText || !view.CanTranslate || view.HasTranslation {
		t.Fatalf("view should show English source with translate available: %+v", view)
	}
}

func TestReadSkillFileUsesCachedTranslationByDefault(t *testing.T) {
	source := "# Teach\n\nThis skill teaches concepts with patient examples."
	manager := newLoadedSkillManager(t, "teach", "SKILL.md", source, func(t *testing.T, projectRoot, translationPath string) {
		cache := seedTranslationCache(t, projectRoot, translationPath, "skill:teach", "SKILL.md", source, "中文译文", "中文摘要")
		if err := cache.Save(); err != nil {
			t.Fatalf("Save translation cache: %v", err)
		}
	})

	view, err := manager.ReadSkillFile("skill:teach", "SKILL.md")
	if err != nil {
		t.Fatalf("ReadSkillFile returned error: %v", err)
	}
	if view.DisplayText != "中文译文" || view.Summary != "中文摘要" || !view.HasTranslation || !view.CanTranslate {
		t.Fatalf("view should use cached translation: %+v", view)
	}
}

func TestReadSkillFileDoesNotOfferTranslationForChineseSource(t *testing.T) {
	source := "# 教学\n\n这个技能会用耐心示例讲清楚一个概念，适合中文用户直接阅读。"
	manager := newLoadedSkillManager(t, "teach", "SKILL.md", source, nil)

	view, err := manager.ReadSkillFile("skill:teach", "SKILL.md")
	if err != nil {
		t.Fatalf("ReadSkillFile returned error: %v", err)
	}
	if view.Language != translate.LanguageChinese || view.DisplayText != source || view.CanTranslate || view.HasTranslation {
		t.Fatalf("Chinese source should display directly without translate action: %+v", view)
	}
}

func TestReadSkillFileAllowsTranslationForUnknownLanguageSource(t *testing.T) {
	source := "12345\n?!"
	manager := newLoadedSkillManager(t, "teach", "SKILL.md", source, nil)

	view, err := manager.ReadSkillFile("skill:teach", "SKILL.md")
	if err != nil {
		t.Fatalf("ReadSkillFile returned error: %v", err)
	}
	if view.Language != translate.LanguageUnknown || view.DisplayText != source || !view.CanTranslate || view.HasTranslation {
		t.Fatalf("unknown source should stay visible and allow manual translation: %+v", view)
	}
}

func TestTranslateSkillFileSavesCacheAndReloads(t *testing.T) {
	source := "# Teach\n\nThis skill teaches concepts with patient examples."
	dir := t.TempDir()
	storePath := filepath.Join(dir, "data", "store.json")
	scanPath := filepath.Join(dir, "data", "last-scan.json")
	translationPath := filepath.Join(dir, "data", "translation-cache.json")
	skillDir := writeSkillFile(t, dir, "teach", "SKILL.md", source)
	manager := NewManager(storePath, scanPath, translationPath, "")
	if err := manager.Load(); err != nil {
		t.Fatalf("Load returned error: %v", err)
	}
	manager.Scan = scanner.Result{Abilities: []domain.Ability{{ID: "skill:teach", Kind: domain.KindSkill, Name: "teach", Directory: skillDir}}}

	translator := ai.Translator{
		Command: fakeTranslatorCommand(t, dir, `{"translatedText":"中文译文","summary":"中文摘要"}`, 0),
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}
	view, err := manager.TranslateSkillFile(context.Background(), translator, "skill:teach", "SKILL.md")
	if err != nil {
		t.Fatalf("TranslateSkillFile returned error: %v", err)
	}
	if view.DisplayText != "中文译文" || view.Summary != "中文摘要" || !view.HasTranslation {
		t.Fatalf("translated view = %+v", view)
	}

	reloaded := NewManager(storePath, scanPath, translationPath, "")
	reloaded.Scan = manager.Scan
	if err := reloaded.Load(); err != nil {
		t.Fatalf("Reload Load returned error: %v", err)
	}
	reloaded.Scan = manager.Scan
	cached, err := reloaded.ReadSkillFile("skill:teach", "SKILL.md")
	if err != nil {
		t.Fatalf("ReadSkillFile after reload returned error: %v", err)
	}
	if cached.DisplayText != "中文译文" || cached.Summary != "中文摘要" || !cached.HasTranslation {
		t.Fatalf("reloaded view should hit translation cache: %+v", cached)
	}
}

func TestTranslateSkillFileRejectsChineseSourceWithoutCallingTranslator(t *testing.T) {
	source := "# 教学\n\n这个技能会用耐心示例讲清楚一个概念，适合中文用户直接阅读。"
	dir := t.TempDir()
	storePath := filepath.Join(dir, "data", "store.json")
	scanPath := filepath.Join(dir, "data", "last-scan.json")
	translationPath := filepath.Join(dir, "data", "translation-cache.json")
	skillDir := writeSkillFile(t, dir, "teach", "SKILL.md", source)
	manager := NewManager(storePath, scanPath, translationPath, "")
	if err := manager.Load(); err != nil {
		t.Fatalf("Load returned error: %v", err)
	}
	manager.Scan = scanner.Result{Abilities: []domain.Ability{{ID: "skill:teach", Kind: domain.KindSkill, Name: "teach", Directory: skillDir}}}
	markerPath := filepath.Join(dir, "translator-called.txt")
	translator := ai.Translator{
		Command: fakeTranslatorCommandWithMarker(t, dir, markerPath, `{"translatedText":"不应写入","summary":"不应写入"}`, 0),
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}

	_, err := manager.TranslateSkillFile(context.Background(), translator, "skill:teach", "SKILL.md")
	if err == nil || !strings.Contains(err.Error(), "中文") {
		t.Fatalf("TranslateSkillFile should reject Chinese source with clear error, got %v", err)
	}
	if _, statErr := os.Stat(markerPath); !os.IsNotExist(statErr) {
		t.Fatalf("translator command should not be called for Chinese source, marker stat err = %v", statErr)
	}
	if got, ok := manager.Translations.Get("skill:teach", "SKILL.md", source); ok {
		t.Fatalf("Chinese source should not be written to translation cache: %#v", got)
	}
}

func TestSkillFileAPIsRejectUnknownNonSkillAndEmptyDirectory(t *testing.T) {
	dir := t.TempDir()
	manager := NewManager(filepath.Join(dir, "data", "store.json"), filepath.Join(dir, "data", "last-scan.json"), filepath.Join(dir, "data", "translation-cache.json"), "")
	manager.Scan = scanner.Result{Abilities: []domain.Ability{
		{ID: "plugin:demo", Kind: domain.KindPlugin, Name: "demo", Directory: dir},
		{ID: "skill:empty", Kind: domain.KindSkill, Name: "empty"},
	}}
	if err := manager.Load(); err != nil {
		t.Fatalf("Load returned error: %v", err)
	}
	manager.Scan.Abilities = []domain.Ability{
		{ID: "plugin:demo", Kind: domain.KindPlugin, Name: "demo", Directory: dir},
		{ID: "skill:empty", Kind: domain.KindSkill, Name: "empty"},
	}

	if _, err := manager.ReadSkillFile("missing", "SKILL.md"); err == nil {
		t.Fatalf("unknown skill should return error")
	}
	if _, err := manager.ListSkillFiles("plugin:demo"); err == nil {
		t.Fatalf("non skill should return error")
	}
	if _, err := manager.ListSkillFiles("skill:empty"); err == nil {
		t.Fatalf("skill without directory should return error")
	}
}

func newLoadedSkillManager(t *testing.T, name, relativePath, content string, seed func(*testing.T, string, string)) *Manager {
	t.Helper()
	dir := t.TempDir()
	storePath := filepath.Join(dir, "data", "store.json")
	scanPath := filepath.Join(dir, "data", "last-scan.json")
	translationPath := filepath.Join(dir, "data", "translation-cache.json")
	skillDir := writeSkillFile(t, dir, name, relativePath, content)
	if seed != nil {
		seed(t, dir, translationPath)
	}
	manager := NewManager(storePath, scanPath, translationPath, "")
	manager.Scan = scanner.Result{Abilities: []domain.Ability{{ID: "skill:" + name, Kind: domain.KindSkill, Name: name, Directory: skillDir}}}
	if err := manager.Load(); err != nil {
		t.Fatalf("Load returned error: %v", err)
	}
	manager.Scan.Abilities = []domain.Ability{{ID: "skill:" + name, Kind: domain.KindSkill, Name: name, Directory: skillDir}}
	return manager
}

func writeSkillFile(t *testing.T, projectRoot, name, relativePath, content string) string {
	t.Helper()
	skillDir := filepath.Join(projectRoot, "skills", name)
	path := filepath.Join(skillDir, filepath.FromSlash(relativePath))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("MkdirAll skill dir: %v", err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("WriteFile skill file: %v", err)
	}
	return skillDir
}

func seedTranslationCache(t *testing.T, projectRoot, translationPath, skillID, relativePath, source, translated, summary string) *translate.Cache {
	t.Helper()
	cache, err := translate.LoadCache(projectRoot, translationPath)
	if err != nil {
		t.Fatalf("LoadCache: %v", err)
	}
	if err := cache.Set(translate.Entry{
		SkillID:        skillID,
		RelativePath:   relativePath,
		ContentHash:    translate.HashText(source),
		SourceLanguage: translate.DetectLanguage(source),
		TranslatedText: translated,
		Summary:        summary,
		UpdatedAt:      time.Now(),
	}); err != nil {
		t.Fatalf("Set translation cache: %v", err)
	}
	return cache
}

func fakeTranslatorCommand(t *testing.T, dir string, stdout string, exitCode int) string {
	t.Helper()
	if runtime.GOOS == "windows" {
		path := filepath.Join(dir, "fake-translator.cmd")
		content := "@echo off\r\n"
		content += "echo " + stdout + "\r\n"
		content += "exit /b " + string(rune('0'+exitCode)) + "\r\n"
		if err := os.WriteFile(path, []byte(content), 0o755); err != nil {
			t.Fatalf("WriteFile fake translator command: %v", err)
		}
		return path
	}
	path := filepath.Join(dir, "fake-translator.sh")
	content := "#!/bin/sh\nprintf '%s\\n' '" + stdout + "'\nexit " + string(rune('0'+exitCode)) + "\n"
	if err := os.WriteFile(path, []byte(content), 0o755); err != nil {
		t.Fatalf("WriteFile fake translator command: %v", err)
	}
	return path
}

func fakeTranslatorCommandWithMarker(t *testing.T, dir, markerPath, stdout string, exitCode int) string {
	t.Helper()
	if runtime.GOOS == "windows" {
		path := filepath.Join(dir, "fake-translator-marker.cmd")
		content := "@echo off\r\n"
		content += "echo called > \"" + markerPath + "\"\r\n"
		content += "echo " + stdout + "\r\n"
		content += "exit /b " + string(rune('0'+exitCode)) + "\r\n"
		if err := os.WriteFile(path, []byte(content), 0o755); err != nil {
			t.Fatalf("WriteFile fake translator marker command: %v", err)
		}
		return path
	}
	path := filepath.Join(dir, "fake-translator-marker.sh")
	content := "#!/bin/sh\nprintf '%s\\n' called > '" + markerPath + "'\nprintf '%s\\n' '" + stdout + "'\nexit " + string(rune('0'+exitCode)) + "\n"
	if err := os.WriteFile(path, []byte(content), 0o755); err != nil {
		t.Fatalf("WriteFile fake translator marker command: %v", err)
	}
	return path
}
