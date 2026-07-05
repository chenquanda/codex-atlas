package app

import (
	"os"
	"path/filepath"
	"testing"

	"codex-atlas/internal/domain"
	"codex-atlas/internal/scanner"
)

func TestManagerMergesSearchesAndKeepsHiddenOutByDefault(t *testing.T) {
	manager := NewManager(filepath.Join(t.TempDir(), "store.json"), filepath.Join(t.TempDir(), "scan.json"), "")
	manager.Scan = scanner.Result{Abilities: []domain.Ability{
		{ID: "skill:teach", Kind: domain.KindSkill, Name: "teach", Description: "Teach concepts"},
		{ID: "skill:debug", Kind: domain.KindSkill, Name: "debug", Description: "Debug bugs"},
	}}
	manager.State.Users["skill:teach"] = domain.UserData{Alias: "教学", Tags: []string{"常用"}}
	manager.State.Users["skill:debug"] = domain.UserData{Hidden: true}
	manager.State.AI["skill:teach"] = domain.AIData{Summary: "讲清楚一个概念", Tags: []string{"写作"}}
	manager.Stats["skill:teach"] = domain.Stats{UsageCount: 4, Known: true}

	visible := manager.Abilities(domain.KindSkill, "", false)
	if len(visible) != 1 {
		t.Fatalf("visible abilities = %d, want 1", len(visible))
	}
	if visible[0].DisplayName != "教学" || visible[0].Stats.UsageCount != 4 || !visible[0].Stats.Known {
		t.Fatalf("merged ability lost user or stats data: %+v", visible[0])
	}

	search := manager.Abilities(domain.KindSkill, "写作", false)
	if len(search) != 1 || search[0].ID != "skill:teach" {
		t.Fatalf("search by AI tag failed: %#v", search)
	}

	withHidden := manager.Abilities(domain.KindSkill, "", true)
	if len(withHidden) != 2 {
		t.Fatalf("withHidden abilities = %d, want 2", len(withHidden))
	}
}

func TestManagerPersistsStatsCache(t *testing.T) {
	dir := t.TempDir()
	codexHome := filepath.Join(dir, "codex")
	sessionPath := filepath.Join(codexHome, "sessions", "2026", "07", "02", "a.jsonl")
	if err := os.MkdirAll(filepath.Dir(sessionPath), 0o755); err != nil {
		t.Fatalf("MkdirAll session dir: %v", err)
	}
	if err := os.WriteFile(sessionPath, []byte(`{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"请使用 $teach 和 $teach"}]}}`), 0o644); err != nil {
		t.Fatalf("WriteFile session: %v", err)
	}

	storePath := filepath.Join(dir, "data", "store.json")
	scanPath := filepath.Join(dir, "data", "last-scan.json")
	manager := NewManager(storePath, scanPath, codexHome)
	manager.Scan = scanner.Result{Abilities: []domain.Ability{
		{ID: "skill:teach", Kind: domain.KindSkill, Name: "teach"},
	}}

	if err := manager.RefreshStats(); err != nil {
		t.Fatalf("RefreshStats returned error: %v", err)
	}

	reloaded := NewManager(storePath, scanPath, codexHome)
	if err := reloaded.Load(); err != nil {
		t.Fatalf("Load returned error: %v", err)
	}
	if got := reloaded.Stats["skill:teach"].UsageCount; got != 2 {
		t.Fatalf("cached teach usage = %d, want 2", got)
	}
	if !reloaded.Stats["skill:teach"].Known {
		t.Fatalf("cached teach stats should be known: %+v", reloaded.Stats["skill:teach"])
	}
}

func TestManagerTreatsLegacyStatsCacheAsUnknown(t *testing.T) {
	dir := t.TempDir()
	storePath := filepath.Join(dir, "data", "store.json")
	scanPath := filepath.Join(dir, "data", "last-scan.json")
	statsPath := filepath.Join(dir, "data", "stats-cache.json")
	if err := os.MkdirAll(filepath.Dir(statsPath), 0o755); err != nil {
		t.Fatalf("MkdirAll stats dir: %v", err)
	}
	if err := os.WriteFile(statsPath, []byte(`{"skill:teach":{"sessionsUsed":8,"mentions":30}}`), 0o644); err != nil {
		t.Fatalf("WriteFile legacy stats cache: %v", err)
	}

	manager := NewManager(storePath, scanPath, "")
	if err := manager.Load(); err != nil {
		t.Fatalf("Load should ignore legacy stats cache, got: %v", err)
	}
	if got := manager.Stats["skill:teach"]; got.Known || got.UsageCount != 0 {
		t.Fatalf("legacy cache should be unknown zero stats, got %+v", got)
	}
}

func TestManagerIgnoresCorruptStatsCache(t *testing.T) {
	dir := t.TempDir()
	storePath := filepath.Join(dir, "data", "store.json")
	scanPath := filepath.Join(dir, "data", "last-scan.json")
	statsPath := filepath.Join(dir, "data", "stats-cache.json")
	if err := os.MkdirAll(filepath.Dir(statsPath), 0o755); err != nil {
		t.Fatalf("MkdirAll stats dir: %v", err)
	}
	if err := os.WriteFile(statsPath, []byte(`{bad json`), 0o644); err != nil {
		t.Fatalf("WriteFile stats cache: %v", err)
	}

	manager := NewManager(storePath, scanPath, "")
	if err := manager.Load(); err != nil {
		t.Fatalf("Load should ignore corrupt stats cache, got: %v", err)
	}
	if len(manager.Stats) != 0 {
		t.Fatalf("Stats after corrupt cache = %#v, want empty map", manager.Stats)
	}
}

func TestManagerSetUserDataPersistsManualFields(t *testing.T) {
	dir := t.TempDir()
	manager := NewManager(filepath.Join(dir, "store.json"), filepath.Join(dir, "scan.json"), "")

	if err := manager.UpdateUser("skill:teach", func(user *domain.UserData) {
		user.Alias = "教学"
		user.Favorite = true
		user.CustomTemplates = []domain.CallTemplate{{Title: "模板", Text: "请使用 $teach 教我"}}
	}); err != nil {
		t.Fatalf("UpdateUser returned error: %v", err)
	}

	reloaded := NewManager(filepath.Join(dir, "store.json"), filepath.Join(dir, "scan.json"), "")
	if err := reloaded.Load(); err != nil {
		t.Fatalf("Load returned error: %v", err)
	}
	user := reloaded.State.Users["skill:teach"]
	if user.Alias != "教学" || !user.Favorite || len(user.CustomTemplates) != 1 {
		t.Fatalf("manual data not persisted: %+v", user)
	}
}
