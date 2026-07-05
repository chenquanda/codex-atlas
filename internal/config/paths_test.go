package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestResolveWorkDirUsesProjectRootForProjectDistExe(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "go.mod"), []byte("module test"), 0o644); err != nil {
		t.Fatalf("WriteFile go.mod: %v", err)
	}
	exePath := filepath.Join(root, "dist", "codex-atlas.exe")
	if err := os.MkdirAll(filepath.Dir(exePath), 0o755); err != nil {
		t.Fatalf("MkdirAll dist: %v", err)
	}

	got := resolveWorkDir(filepath.Join(root, "elsewhere"), exePath)
	if got != root {
		t.Fatalf("resolveWorkDir for project dist = %q, want %q", got, root)
	}
}

func TestDefaultPathsSetsTranslationPathUnderWorkDirData(t *testing.T) {
	paths, err := DefaultPaths()
	if err != nil {
		t.Fatalf("DefaultPaths() error = %v", err)
	}

	want := filepath.Join(paths.WorkDir, "data", "translation-cache.json")
	if paths.TranslationPath != want {
		t.Fatalf("TranslationPath = %q, want %q", paths.TranslationPath, want)
	}
}

func TestResolveWorkDirUsesExeDirForPortableBundle(t *testing.T) {
	portable := t.TempDir()
	exePath := filepath.Join(portable, "codex-atlas.exe")
	if err := os.WriteFile(filepath.Join(portable, "codex-atlas.exe.manifest"), []byte("manifest"), 0o644); err != nil {
		t.Fatalf("WriteFile manifest: %v", err)
	}

	got := resolveWorkDir(filepath.Join(portable, "other-cwd"), exePath)
	if got != portable {
		t.Fatalf("resolveWorkDir for portable exe = %q, want %q", got, portable)
	}
}
