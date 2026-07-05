package ai

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"testing"
	"time"
)

func TestTranslatorRunsCommandAndParsesResponse(t *testing.T) {
	dir := t.TempDir()
	command := fakeCommand(t, dir, `{"translatedText":"中文译文","summary":"中文摘要"}`, 0)
	outputPath := filepath.Join(dir, ".tmp", "translation-output.json")
	translator := Translator{
		Command:    command,
		WorkDir:    dir,
		OutputPath: outputPath,
		Timeout:    2 * time.Second,
	}

	got, err := translator.Translate(context.Background(), TranslateRequest{
		SkillName: "teach",
		Path:      "SKILL.md",
		Text:      "This skill teaches concepts.",
	})
	if err != nil {
		t.Fatalf("Translate returned error: %v", err)
	}
	if got.TranslatedText != "中文译文" || got.Summary != "中文摘要" {
		t.Fatalf("response = %+v", got)
	}
	if _, err := os.Stat(outputPath); err != nil {
		t.Fatalf("translation output should be written inside project tmp dir: %v", err)
	}
}

func TestTranslatorDefaultOutputPathUsesFixedTmpFile(t *testing.T) {
	dir := t.TempDir()
	outputPath := filepath.Join(dir, ".tmp", "translation-output.json")
	translator := Translator{
		Command: fakeCommand(t, dir, `{"translatedText":"默认译文","summary":"默认摘要"}`, 0),
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}

	got, err := translator.Translate(context.Background(), TranslateRequest{
		SkillName: "teach",
		Path:      "SKILL.md",
		Text:      "This skill teaches concepts.",
	})
	if err != nil {
		t.Fatalf("Translate returned error: %v", err)
	}
	if got.TranslatedText != "默认译文" || got.Summary != "默认摘要" {
		t.Fatalf("response = %+v", got)
	}
	if _, err := os.Stat(outputPath); err != nil {
		t.Fatalf("default translation output should use fixed tmp file: %v", err)
	}
}

func TestTranslatorRejectsOutputPathOutsideWorkTmp(t *testing.T) {
	dir := t.TempDir()
	translator := Translator{
		Command:    fakeCommand(t, dir, `{"translatedText":"不应执行","summary":"不应执行"}`, 0),
		WorkDir:    dir,
		OutputPath: filepath.Join(dir, "translation-output.json"),
		Timeout:    2 * time.Second,
	}

	if _, err := translator.Translate(context.Background(), TranslateRequest{SkillName: "teach", Path: "SKILL.md", Text: "English text"}); err == nil {
		t.Fatalf("Translate should reject OutputPath outside WorkDir .tmp")
	}
}

func TestTranslatorRejectsTmpLinkOutsideWorkDir(t *testing.T) {
	dir := t.TempDir()
	outside := t.TempDir()
	tmpPath := filepath.Join(dir, ".tmp")
	createDirectoryLinkForTest(t, outside, tmpPath)

	translator := Translator{
		Command: fakeCommand(t, dir, `{"translatedText":"不应写出项目","summary":"不应写出项目"}`, 0),
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}

	if _, err := translator.Translate(context.Background(), TranslateRequest{SkillName: "teach", Path: "SKILL.md", Text: "English text"}); err == nil {
		t.Fatalf("Translate should reject WorkDir .tmp when it resolves outside the project")
	}
	if _, err := os.Stat(filepath.Join(outside, "translation-output.json")); err == nil {
		t.Fatalf("Translate wrote output through a linked .tmp directory outside WorkDir")
	}
}

func TestTranslatorDefaultOutputDoesNotReadStaleFixedFile(t *testing.T) {
	dir := t.TempDir()
	tmpDir := filepath.Join(dir, ".tmp")
	if err := os.MkdirAll(tmpDir, 0o755); err != nil {
		t.Fatalf("MkdirAll tmp dir: %v", err)
	}
	stalePath := filepath.Join(tmpDir, "translation-output.json")
	if err := os.WriteFile(stalePath, []byte(`{"translatedText":"旧译文","summary":"旧摘要"}`), 0o644); err != nil {
		t.Fatalf("WriteFile stale output: %v", err)
	}
	translator := Translator{
		Command: fakeCommand(t, dir, `{"translatedText":"新译文","summary":"新摘要"}`, 0),
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}

	got, err := translator.Translate(context.Background(), TranslateRequest{SkillName: "teach", Path: "SKILL.md", Text: "English text"})
	if err != nil {
		t.Fatalf("Translate returned error: %v", err)
	}
	if got.TranslatedText != "新译文" || got.Summary != "新摘要" {
		t.Fatalf("Translate read stale fixed output, got %+v", got)
	}
}

func TestTranslatorReturnsClearErrorWhenCommandFails(t *testing.T) {
	dir := t.TempDir()
	translator := Translator{
		Command: fakeCommand(t, dir, `{"translatedText":"never","summary":"never"}`, 2),
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}

	if _, err := translator.Translate(context.Background(), TranslateRequest{SkillName: "teach", Path: "SKILL.md", Text: "English text"}); err == nil {
		t.Fatalf("Translate should return error for failing command")
	}
}

func TestTranslatorRejectsBadJSON(t *testing.T) {
	dir := t.TempDir()
	translator := Translator{
		Command: fakeCommand(t, dir, `not json`, 0),
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}

	if _, err := translator.Translate(context.Background(), TranslateRequest{SkillName: "teach", Path: "SKILL.md", Text: "English text"}); err == nil {
		t.Fatalf("Translate should reject non JSON output")
	}
}

func TestTranslatorRequiresTranslatedText(t *testing.T) {
	dir := t.TempDir()
	translator := Translator{
		Command: fakeCommand(t, dir, `{"summary":"只有摘要"}`, 0),
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}

	if _, err := translator.Translate(context.Background(), TranslateRequest{SkillName: "teach", Path: "SKILL.md", Text: "English text"}); err == nil {
		t.Fatalf("Translate should reject response without translatedText")
	}
}

func createDirectoryLinkForTest(t *testing.T, target, link string) {
	t.Helper()
	if err := os.Symlink(target, link); err == nil {
		return
	}
	if runtime.GOOS == "windows" {
		cmd := exec.Command("cmd", "/c", "mklink", "/J", link, target)
		if output, err := cmd.CombinedOutput(); err == nil {
			return
		} else {
			t.Skipf("cannot create directory symlink or junction: %v; %s", err, string(output))
		}
	}
	t.Skip("cannot create directory symlink on this system")
}
