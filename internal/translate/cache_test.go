package translate

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestCacheSavesReloadsAndMissesWhenContentHashChanges(t *testing.T) {
	root := newTranslateTempDir(t)
	cachePath := filepath.Join(root, "data", "translation-cache.json")

	cache, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() error = %v", err)
	}
	text := "Use this skill to teach a topic."
	if err := cache.Set(Entry{
		SkillID:        "skill:teach",
		RelativePath:   "references\\guide.md",
		ContentHash:    HashText(text),
		SourceLanguage: LanguageEnglish,
		TranslatedText: "使用这个技能来教学一个主题。",
	}); err != nil {
		t.Fatalf("Set() error = %v", err)
	}
	if err := cache.Save(); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	reloaded, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() after Save error = %v", err)
	}
	if got, ok := reloaded.Get("skill:teach", "references/guide.md", text); !ok || got.TranslatedText != "使用这个技能来教学一个主题。" {
		t.Fatalf("Get() = %#v, %v, want saved translation hit", got, ok)
	}
	if got, ok := reloaded.Get("skill:teach", "references/guide.md", "Changed content."); ok {
		t.Fatalf("Get() with changed content = %#v, true, want miss", got)
	}

	data, err := os.ReadFile(cachePath)
	if err != nil {
		t.Fatalf("ReadFile() error = %v", err)
	}
	if strings.Contains(string(data), root) || strings.Contains(string(data), cachePath) {
		t.Fatalf("cache JSON persisted runtime paths: %s", data)
	}
	if !json.Valid(data) {
		t.Fatalf("cache JSON is invalid: %s", data)
	}
}

func TestCachePreservesFullEntryAfterReload(t *testing.T) {
	root := newTranslateTempDir(t)
	cachePath := filepath.Join(root, "data", "translation-cache.json")
	updatedAt := time.Date(2026, 7, 5, 10, 30, 0, 0, time.UTC)

	cache, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() error = %v", err)
	}
	text := "Use this skill when designing lessons."
	if err := cache.Set(Entry{
		SkillID:        "skill:teach",
		RelativePath:   "SKILL.md",
		ContentHash:    HashText(text),
		SourceLanguage: LanguageEnglish,
		TranslatedText: "设计课程时使用这个技能。",
		Summary:        "课程设计助手",
		UpdatedAt:      updatedAt,
	}); err != nil {
		t.Fatalf("Set() error = %v", err)
	}
	if err := cache.Save(); err != nil {
		t.Fatalf("Save() error = %v", err)
	}

	reloaded, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() after Save error = %v", err)
	}
	got, ok := reloaded.Get("skill:teach", "SKILL.md", text)
	if !ok {
		t.Fatal("Get() after reload missed saved entry")
	}
	if got.TranslatedText != "设计课程时使用这个技能。" {
		t.Fatalf("TranslatedText = %q, want %q", got.TranslatedText, "设计课程时使用这个技能。")
	}
	if got.Summary != "课程设计助手" {
		t.Fatalf("Summary = %q, want %q", got.Summary, "课程设计助手")
	}
	if got.SourceLanguage != LanguageEnglish {
		t.Fatalf("SourceLanguage = %q, want %q", got.SourceLanguage, LanguageEnglish)
	}
	if !got.UpdatedAt.Equal(updatedAt) {
		t.Fatalf("UpdatedAt = %s, want %s", got.UpdatedAt, updatedAt)
	}
}

func TestCacheSaveTwiceReplacesExistingJSON(t *testing.T) {
	root := newTranslateTempDir(t)
	cachePath := filepath.Join(root, "data", "translation-cache.json")

	cache, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() error = %v", err)
	}
	text := "hello"
	if err := cache.Set(Entry{
		SkillID:        "skill:teach",
		RelativePath:   "docs/lesson.md",
		ContentHash:    HashText(text),
		TranslatedText: "第一版",
		UpdatedAt:      time.Date(2026, 7, 5, 9, 0, 0, 0, time.UTC),
	}); err != nil {
		t.Fatalf("Set() first version error = %v", err)
	}
	if err := cache.Save(); err != nil {
		t.Fatalf("first Save() error = %v", err)
	}

	if err := cache.Set(Entry{
		SkillID:        "skill:teach",
		RelativePath:   "docs/lesson.md",
		ContentHash:    HashText(text),
		TranslatedText: "第二版",
		Summary:        "新版摘要",
		UpdatedAt:      time.Date(2026, 7, 5, 11, 0, 0, 0, time.UTC),
	}); err != nil {
		t.Fatalf("Set() second version error = %v", err)
	}
	if err := cache.Save(); err != nil {
		t.Fatalf("second Save() error = %v", err)
	}

	data, err := os.ReadFile(cachePath)
	if err != nil {
		t.Fatalf("ReadFile() error = %v", err)
	}
	if !json.Valid(data) {
		t.Fatalf("cache JSON is invalid after second Save(): %s", data)
	}
	reloaded, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() after second Save error = %v", err)
	}
	got, ok := reloaded.Get("skill:teach", "docs/lesson.md", text)
	if !ok {
		t.Fatal("Get() after second Save missed saved entry")
	}
	if got.TranslatedText != "第二版" || got.Summary != "新版摘要" {
		t.Fatalf("entry after second Save = %#v, want second version", got)
	}
}

func TestLoadCacheReturnsEmptyForMissingAndCorruptFiles(t *testing.T) {
	root := newTranslateTempDir(t)
	cachePath := filepath.Join(root, "data", "translation-cache.json")

	missing, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() missing error = %v", err)
	}
	if len(missing.Entries) != 0 {
		t.Fatalf("missing cache Entries length = %d, want 0", len(missing.Entries))
	}

	if err := os.MkdirAll(filepath.Dir(cachePath), 0o755); err != nil {
		t.Fatalf("MkdirAll() error = %v", err)
	}
	if err := os.WriteFile(cachePath, []byte("{bad json"), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}
	corrupt, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() corrupt error = %v", err)
	}
	if len(corrupt.Entries) != 0 {
		t.Fatalf("corrupt cache Entries length = %d, want 0", len(corrupt.Entries))
	}
}

func TestLoadCacheReturnsErrorWhenCachePathIsDirectory(t *testing.T) {
	root := newTranslateTempDir(t)
	cachePath := filepath.Join(root, "data", "translation-cache.json")
	if err := os.MkdirAll(cachePath, 0o755); err != nil {
		t.Fatalf("MkdirAll() cache path error = %v", err)
	}

	if _, err := LoadCache(root, cachePath); err == nil {
		t.Fatal("LoadCache() with directory cache path error = nil, want error")
	}
}

func TestLoadCacheFallsBackToBackupWhenMainCacheMissing(t *testing.T) {
	root := newTranslateTempDir(t)
	cachePath := filepath.Join(root, "data", "translation-cache.json")

	cache, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() error = %v", err)
	}
	text := "backup source"
	if err := cache.Set(Entry{
		SkillID:        "skill:teach",
		RelativePath:   "docs/backup.md",
		ContentHash:    HashText(text),
		TranslatedText: "备份内容",
		UpdatedAt:      time.Date(2026, 7, 5, 12, 0, 0, 0, time.UTC),
	}); err != nil {
		t.Fatalf("Set() error = %v", err)
	}
	if err := cache.Save(); err != nil {
		t.Fatalf("Save() error = %v", err)
	}
	if err := os.Rename(cachePath, cachePath+".bak"); err != nil {
		t.Fatalf("Rename() main cache to backup error = %v", err)
	}

	recovered, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() with backup error = %v", err)
	}
	got, ok := recovered.Get("skill:teach", "docs/backup.md", text)
	if !ok || got.TranslatedText != "备份内容" {
		t.Fatalf("Get() from backup = %#v, %v, want backup hit", got, ok)
	}
}

func TestLoadCacheRejectsPathOutsideProjectRoot(t *testing.T) {
	root := newTranslateTempDir(t)
	outside := filepath.Join(filepath.Dir(root), "outside-cache.json")

	if _, err := LoadCache(root, outside); !errors.Is(err, ErrCachePathOutside) {
		t.Fatalf("LoadCache() outside error = %v, want %v", err, ErrCachePathOutside)
	}
}

func TestLoadCacheRejectsSymlinkedDataDirectoryOutsideProjectRoot(t *testing.T) {
	root := newTranslateTempDir(t)
	outside := newTranslateTempDir(t)
	dataLink := filepath.Join(root, "data")
	outsideAbs, err := filepath.Abs(outside)
	if err != nil {
		t.Fatalf("Abs() outside error = %v", err)
	}
	if err := os.Symlink(outsideAbs, dataLink); err != nil {
		t.Skipf("当前环境不支持创建 symlink，跳过 data 目录逃逸测试: %v", err)
	}

	cachePath := filepath.Join(dataLink, "translation-cache.json")
	if _, err := LoadCache(root, cachePath); !errors.Is(err, ErrCachePathOutside) {
		t.Fatalf("LoadCache() symlinked data error = %v, want %v", err, ErrCachePathOutside)
	}
}

func TestCacheSetRejectsUnsafeRelativePaths(t *testing.T) {
	root := newTranslateTempDir(t)
	cache, err := LoadCache(root, filepath.Join(root, "data", "translation-cache.json"))
	if err != nil {
		t.Fatalf("LoadCache() error = %v", err)
	}

	for _, relativePath := range []string{
		"",
		"../x.md",
		"..\\x.md",
		filepath.Join(root, "x.md"),
	} {
		err := cache.Set(Entry{
			SkillID:        "skill:teach",
			RelativePath:   relativePath,
			ContentHash:    HashText("unsafe"),
			TranslatedText: "不应写入",
		})
		if !errors.Is(err, ErrCachePathOutside) {
			t.Fatalf("Set(%q) error = %v, want %v", relativePath, err, ErrCachePathOutside)
		}
	}
	if len(cache.Entries) != 0 {
		t.Fatalf("unsafe Set() wrote entries: %#v", cache.Entries)
	}
}

func TestCacheGetMissesUnsafeRelativePaths(t *testing.T) {
	root := newTranslateTempDir(t)
	cache, err := LoadCache(root, filepath.Join(root, "data", "translation-cache.json"))
	if err != nil {
		t.Fatalf("LoadCache() error = %v", err)
	}
	text := "safe"
	if err := cache.Set(Entry{
		SkillID:        "skill:teach",
		RelativePath:   "docs/safe.md",
		ContentHash:    HashText(text),
		TranslatedText: "安全内容",
	}); err != nil {
		t.Fatalf("Set() error = %v", err)
	}

	for _, relativePath := range []string{"", "../safe.md", "..\\safe.md", filepath.Join(root, "safe.md")} {
		if got, ok := cache.Get("skill:teach", relativePath, text); ok {
			t.Fatalf("Get(%q) = %#v, true, want miss", relativePath, got)
		}
		if got, ok := cache.GetByHash("skill:teach", relativePath, HashText(text)); ok {
			t.Fatalf("GetByHash(%q) = %#v, true, want miss", relativePath, got)
		}
	}
}

func TestCacheNormalizesWindowsBackslashKeys(t *testing.T) {
	root := newTranslateTempDir(t)
	cachePath := filepath.Join(root, "data", "translation-cache.json")

	cache, err := LoadCache(root, cachePath)
	if err != nil {
		t.Fatalf("LoadCache() error = %v", err)
	}
	text := "hello"
	hash := HashText(text)
	if err := cache.Set(Entry{
		SkillID:        "skill:teach",
		RelativePath:   "docs\\lesson.md",
		ContentHash:    hash,
		TranslatedText: "你好",
	}); err != nil {
		t.Fatalf("Set() error = %v", err)
	}

	if _, ok := cache.Get("skill:teach", "docs/lesson.md", text); !ok {
		t.Fatal("Get() with slash path missed after Set() with backslash path")
	}
	if _, ok := cache.Entries["skill:teach|docs/lesson.md|"+hash]; !ok {
		t.Fatalf("Entries key was not normalized to slash path: %#v", cache.Entries)
	}
}

func newTranslateTempDir(t *testing.T) string {
	t.Helper()

	base := filepath.Join("..", "..", ".tmp", "translate-tests")
	if err := os.MkdirAll(base, 0o755); err != nil {
		t.Fatalf("MkdirAll() error = %v", err)
	}
	dir, err := os.MkdirTemp(base, t.Name()+"-")
	if err != nil {
		t.Fatalf("MkdirTemp() error = %v", err)
	}
	t.Cleanup(func() {
		if err := os.RemoveAll(dir); err != nil {
			t.Fatalf("RemoveAll() error = %v", err)
		}
	})
	return dir
}
