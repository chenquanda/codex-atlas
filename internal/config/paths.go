package config

import (
	"os"
	"path/filepath"
	"strings"
)

type Paths struct {
	WorkDir   string
	StorePath string
	CachePath string
	CodexHome string
}

// DefaultPaths 把应用自己的数据固定到当前工作目录，保持便携并满足项目内写入约束。
func DefaultPaths() (Paths, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return Paths{}, err
	}
	exePath, err := os.Executable()
	if err != nil {
		exePath = ""
	}
	workDir := resolveWorkDir(cwd, exePath)
	codexHome := strings.TrimSpace(os.Getenv("CODEX_HOME"))
	if codexHome == "" {
		codexHome = filepath.Join(os.Getenv("USERPROFILE"), ".codex")
	}
	return Paths{
		WorkDir:   workDir,
		StorePath: filepath.Join(workDir, "data", "atlas-store.json"),
		CachePath: filepath.Join(workDir, "data", "last-scan.json"),
		CodexHome: codexHome,
	}, nil
}

func resolveWorkDir(cwd, exePath string) string {
	exeDir := filepath.Dir(exePath)
	if exePath == "" || exeDir == "." || strings.TrimSpace(exeDir) == "" {
		return cwd
	}
	if filepath.Base(exeDir) == "dist" {
		projectRoot := filepath.Dir(exeDir)
		if looksLikeProjectRoot(projectRoot) {
			return projectRoot
		}
	}
	if hasPortableCompanion(exeDir) {
		return exeDir
	}
	return cwd
}

func looksLikeProjectRoot(path string) bool {
	if _, err := os.Stat(filepath.Join(path, "go.mod")); err == nil {
		return true
	}
	if _, err := os.Stat(filepath.Join(path, "scripts", "build.ps1")); err == nil {
		return true
	}
	return false
}

func hasPortableCompanion(path string) bool {
	for _, name := range []string{"codex-atlas.exe.manifest", "使用说明.md"} {
		if _, err := os.Stat(filepath.Join(path, name)); err == nil {
			return true
		}
	}
	return false
}
