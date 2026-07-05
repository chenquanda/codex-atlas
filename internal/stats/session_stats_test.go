package stats

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"codex-atlas/internal/domain"
)

func TestCollectSkillStatsCountsExplicitUserInvocationsAsUsage(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "sessions", "2026", "07", "01", "a.jsonl"), `{"timestamp":"2026-07-01T10:00:00Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"请使用 $teach，再用 $teach，最后 $teach"}]}}
{"timestamp":"2026-07-01T10:01:00Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"teach 只是普通文本，不计入使用"}]}}`)
	writeFile(t, filepath.Join(root, "sessions", "2026", "07", "02", "b.jsonl"), `{"timestamp":"2026-07-02T11:00:00Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"可用 skills: $teach $diagnosing-bugs"}]}}
{"timestamp":"2026-07-02T11:01:00Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"这次只讨论 teach，不显式调用"}]}}`)
	writeFile(t, filepath.Join(root, "sessions", "2026", "07", "02", "c.jsonl"), `{"timestamp":"2026-07-02T12:00:00Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"请使用 $diagnosing-bugs 排查"}]}}`)

	abilities := []domain.Ability{
		{ID: "skill:teach", Kind: domain.KindSkill, Name: "teach"},
		{ID: "skill:diagnosing-bugs", Kind: domain.KindSkill, Name: "diagnosing-bugs"},
	}

	got, err := CollectSkillStats(filepath.Join(root, "sessions"), abilities)
	if err != nil {
		t.Fatalf("CollectSkillStats returned error: %v", err)
	}

	if got["skill:teach"].UsageCount != 3 {
		t.Fatalf("teach usage = %d, want 3", got["skill:teach"].UsageCount)
	}
	if got["skill:diagnosing-bugs"].UsageCount != 1 {
		t.Fatalf("diagnosing-bugs usage = %d, want 1", got["skill:diagnosing-bugs"].UsageCount)
	}
	if !got["skill:teach"].Known || !got["skill:diagnosing-bugs"].Known {
		t.Fatalf("stats should be marked known: %+v", got)
	}
	if got["skill:teach"].LastUsedAt.IsZero() {
		t.Fatalf("teach LastUsedAt should be set")
	}
}

func TestCollectSkillStatsCountsSkillFileReadsAsUsage(t *testing.T) {
	root := t.TempDir()
	skillPath := filepath.Join(root, "skills", "teach", "SKILL.md")
	writeFile(t, filepath.Join(root, "sessions", "read.jsonl"), `{"timestamp":"2026-07-01T10:00:00Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"Get-Content -LiteralPath '`+escapeForJSON(skillPath)+`'\"}"}}
{"timestamp":"2026-07-01T10:01:00Z","type":"response_item","payload":{"type":"function_call_output","output":"read `+escapeForJSON(skillPath)+` again, should not count"}}`)

	got, err := CollectSkillStats(filepath.Join(root, "sessions"), []domain.Ability{
		{ID: "skill:teach", Kind: domain.KindSkill, Name: "teach", SourcePath: skillPath},
	})
	if err != nil {
		t.Fatalf("CollectSkillStats returned error: %v", err)
	}
	if got["skill:teach"].UsageCount != 1 {
		t.Fatalf("teach usage from SKILL.md read = %d, want 1", got["skill:teach"].UsageCount)
	}
}

func TestCollectSkillStatsHandlesLongSessionLines(t *testing.T) {
	root := t.TempDir()
	longContent := strings.Repeat("长文本", 2_000_000) + " $teach"
	writeFile(t, filepath.Join(root, "long.jsonl"), `{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"`+longContent+`"}]}}`)

	got, err := CollectSkillStats(root, []domain.Ability{{ID: "skill:teach", Kind: domain.KindSkill, Name: "teach"}})
	if err != nil {
		t.Fatalf("CollectSkillStats returned error for long line: %v", err)
	}
	if got["skill:teach"].UsageCount != 1 {
		t.Fatalf("teach usage = %d, want 1", got["skill:teach"].UsageCount)
	}
}

func TestCollectSkillStatsDoesNotCrossCountDuplicatePluginSkillNames(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "sessions", "a.jsonl"), `{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"请使用 $alpha:index、$index 和 $writer"}]}}`)

	abilities := []domain.Ability{
		{ID: "skill:alpha:index", Kind: domain.KindSkill, Name: "alpha:index"},
		{ID: "skill:beta:index", Kind: domain.KindSkill, Name: "beta:index"},
		{ID: "skill:solo:writer", Kind: domain.KindSkill, Name: "solo:writer"},
	}

	got, err := CollectSkillStats(root, abilities)
	if err != nil {
		t.Fatalf("CollectSkillStats returned error: %v", err)
	}
	if got["skill:alpha:index"].UsageCount != 1 {
		t.Fatalf("alpha:index usage = %d, want only full-name match", got["skill:alpha:index"].UsageCount)
	}
	if got["skill:beta:index"].UsageCount != 0 {
		t.Fatalf("beta:index usage = %d, want no short-name cross count", got["skill:beta:index"].UsageCount)
	}
	if got["skill:solo:writer"].UsageCount != 1 {
		t.Fatalf("solo:writer usage = %d, want unique short-name match", got["skill:solo:writer"].UsageCount)
	}
}

func TestMentionIndexMapsFullNamesAndOnlyUniqueShortNames(t *testing.T) {
	index := buildMentionIndex([]domain.Ability{
		{ID: "skill:alpha:index", Kind: domain.KindSkill, Name: "alpha:index"},
		{ID: "skill:beta:index", Kind: domain.KindSkill, Name: "beta:index"},
		{ID: "skill:solo:writer", Kind: domain.KindSkill, Name: "solo:writer"},
		{ID: "skill:teach", Kind: domain.KindSkill, Name: "teach"},
	})

	if got := index.lookup("alpha:index"); len(got) != 1 || got[0] != "skill:alpha:index" {
		t.Fatalf("full plugin lookup = %#v", got)
	}
	if got := index.lookup("index"); len(got) != 0 {
		t.Fatalf("duplicate short name should not match: %#v", got)
	}
	if got := index.lookup("writer"); len(got) != 1 || got[0] != "skill:solo:writer" {
		t.Fatalf("unique short plugin lookup = %#v", got)
	}
	if got := index.lookup("teach"); len(got) != 1 || got[0] != "skill:teach" {
		t.Fatalf("local lookup = %#v", got)
	}
}

func writeFile(t *testing.T, path string, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("MkdirAll(%s): %v", filepath.Dir(path), err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("WriteFile(%s): %v", path, err)
	}
}

func escapeForJSON(path string) string {
	return strings.ReplaceAll(filepath.ToSlash(path), `/`, `\/`)
}
