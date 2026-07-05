package ai

import (
	"context"
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"

	"codex-atlas/internal/domain"
)

func TestOrganizerRunsCommandAndParsesAIData(t *testing.T) {
	dir := t.TempDir()
	command := fakeCommand(t, dir, `{"items":[{"id":"skill:teach","summary":"中文摘要","tags":["教学"],"templates":[{"title":"教学模板","text":"请使用 $teach 教我"}]}]}`, 0)
	outputPath := filepath.Join(dir, ".tmp", "ai-output.json")
	organizer := Organizer{
		Command:    command,
		WorkDir:    dir,
		Timeout:    2 * time.Second,
		OutputPath: outputPath,
	}

	got, err := organizer.Organize(context.Background(), []domain.Ability{{ID: "skill:teach", Name: "teach"}})
	if err != nil {
		t.Fatalf("Organize returned error: %v", err)
	}
	if got["skill:teach"].Summary != "中文摘要" {
		t.Fatalf("summary = %q", got["skill:teach"].Summary)
	}
	if len(got["skill:teach"].Templates) != 1 {
		t.Fatalf("templates = %#v", got["skill:teach"].Templates)
	}
	if _, err := os.Stat(outputPath); err != nil {
		t.Fatalf("AI output file should be written inside project tmp dir: %v", err)
	}
}

func TestOrganizerReturnsClearErrorWhenCommandFails(t *testing.T) {
	dir := t.TempDir()
	command := fakeCommand(t, dir, `bad output`, 3)
	organizer := Organizer{
		Command: command,
		WorkDir: dir,
		Timeout: 2 * time.Second,
	}

	_, err := organizer.Organize(context.Background(), []domain.Ability{{ID: "skill:teach", Name: "teach"}})
	if err == nil {
		t.Fatalf("Organize should return error for failing command")
	}
}

func TestOrganizerRejectsUnknownIDsAndBadSchema(t *testing.T) {
	dir := t.TempDir()
	command := fakeCommand(t, dir, `{"items":[{"id":"skill:other","summary":"越界"}]}`, 0)
	organizer := Organizer{Command: command, WorkDir: dir, Timeout: 2 * time.Second}

	if _, err := organizer.Organize(context.Background(), []domain.Ability{{ID: "skill:teach", Name: "teach"}}); err == nil {
		t.Fatalf("Organize should reject AI items outside the input ID set")
	}

	command = fakeCommand(t, dir, `{}`, 0)
	organizer = Organizer{Command: command, WorkDir: dir, Timeout: 2 * time.Second}
	if _, err := organizer.Organize(context.Background(), []domain.Ability{{ID: "skill:teach", Name: "teach"}}); err == nil {
		t.Fatalf("Organize should reject JSON without items")
	}
}

func fakeCommand(t *testing.T, dir string, stdout string, exitCode int) string {
	t.Helper()
	if runtime.GOOS == "windows" {
		path := filepath.Join(dir, "fake-codex.cmd")
		content := "@echo off\r\n"
		content += "echo " + stdout + "\r\n"
		content += "exit /b " + string(rune('0'+exitCode)) + "\r\n"
		if err := os.WriteFile(path, []byte(content), 0o755); err != nil {
			t.Fatalf("WriteFile fake command: %v", err)
		}
		return path
	}
	path := filepath.Join(dir, "fake-codex.sh")
	content := "#!/bin/sh\nprintf '%s\\n' '" + stdout + "'\nexit " + string(rune('0'+exitCode)) + "\n"
	if err := os.WriteFile(path, []byte(content), 0o755); err != nil {
		t.Fatalf("WriteFile fake command: %v", err)
	}
	return path
}
