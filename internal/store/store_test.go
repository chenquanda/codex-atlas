package store

import (
	"path/filepath"
	"strings"
	"testing"

	"codex-atlas/internal/domain"
)

func TestStoreRoundTripPreservesUserAndAIData(t *testing.T) {
	path := filepath.Join(t.TempDir(), "atlas-store.json")
	s := NewJSONStore(path)
	state := domain.Store{
		Version: 1,
		Users: map[string]domain.UserData{
			"skill:teach": {
				Alias:    "教学",
				Tags:     []string{"常用"},
				Favorite: true,
			},
		},
		AI: map[string]domain.AIData{
			"skill:teach": {
				Summary: "教学摘要",
			},
		},
	}

	if err := s.Save(state); err != nil {
		t.Fatalf("Save returned error: %v", err)
	}
	loaded, err := s.Load()
	if err != nil {
		t.Fatalf("Load returned error: %v", err)
	}

	if loaded.Users["skill:teach"].Alias != "教学" {
		t.Fatalf("user alias not preserved: %+v", loaded.Users["skill:teach"])
	}
	if loaded.AI["skill:teach"].Summary != "教学摘要" {
		t.Fatalf("AI summary not preserved: %+v", loaded.AI["skill:teach"])
	}
}

func TestBoundedStoreRejectsPathOutsideProjectRoot(t *testing.T) {
	root := t.TempDir()
	outside := filepath.Join(t.TempDir(), "atlas-store.json")

	s, err := NewBoundedJSONStore(root, outside)
	if err == nil {
		t.Fatalf("NewBoundedJSONStore should reject outside path, got store: %+v", s)
	}
	if !strings.Contains(err.Error(), "项目目录外") {
		t.Fatalf("error = %q, want project boundary message", err)
	}
}

func TestLoadMissingStoreReturnsInitializedMaps(t *testing.T) {
	s := NewJSONStore(filepath.Join(t.TempDir(), "missing.json"))

	loaded, err := s.Load()
	if err != nil {
		t.Fatalf("Load missing returned error: %v", err)
	}
	if loaded.Users == nil || loaded.AI == nil || loaded.Settings == nil {
		t.Fatalf("store maps not initialized: %+v", loaded)
	}
}
