package scanner

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"codex-atlas/internal/domain"
)

func TestScanAllFindsSkillsPluginsToolsAndApps(t *testing.T) {
	home := t.TempDir()
	writeFile(t, filepath.Join(home, "skills", "demo", "SKILL.md"), `---
name: demo-skill
description: Demo skill description
---

# Demo Skill
`)
	writeFile(t, filepath.Join(home, "plugins", "cache", "remote", "sample", "1.0.0", ".codex-plugin", "plugin.json"), `{
  "name": "sample",
  "version": "1.0.0",
  "description": "Sample plugin",
  "skills": "./skills/",
  "apps": "./.app.json",
  "mcpServers": "./.mcp.json",
  "interface": {
    "displayName": "Sample Plugin",
    "shortDescription": "Short plugin description",
    "capabilities": ["Read", "Write"]
  }
}`)
	writeFile(t, filepath.Join(home, "plugins", "cache", "remote", "sample", "1.0.0", "skills", "worker", "SKILL.md"), `---
name: worker
description: Worker skill
---

# Worker
`)
	writeFile(t, filepath.Join(home, "plugins", "cache", "remote", "sample", "1.0.0", ".app.json"), `{
  "apps": {
    "github": {
      "id": "connector_abc",
      "required": true
    }
  }
}`)
	writeFile(t, filepath.Join(home, "plugins", "cache", "remote", "sample", "1.0.0", ".mcp.json"), `{
  "mcpServers": {
    "sampleTool": {
      "title": "Sample Tool",
      "description": "A sample MCP tool",
      "command": "node"
    }
  }
}`)

	result, err := ScanAll(context.Background(), Options{CodexHome: home})
	if err != nil {
		t.Fatalf("ScanAll returned error: %v", err)
	}

	assertAbility(t, result.Abilities, domain.KindSkill, "demo-skill")
	assertAbility(t, result.Abilities, domain.KindSkill, "sample:worker")
	assertAbility(t, result.Abilities, domain.KindPlugin, "sample")
	assertAbility(t, result.Abilities, domain.KindTool, "sampleTool")
	assertAbility(t, result.Abilities, domain.KindApp, "github")
}

func TestParseSkillMarkdownFallsBackWhenFrontMatterIsMissing(t *testing.T) {
	path := filepath.Join(t.TempDir(), "plain-skill", "SKILL.md")
	writeFile(t, path, "# Plain Skill\n\nA useful plain description.\n")

	parsed, err := ParseSkillFile(path, "", "")
	if err != nil {
		t.Fatalf("ParseSkillFile returned error: %v", err)
	}
	if parsed.Name != "plain-skill" {
		t.Fatalf("Name = %q, want directory fallback", parsed.Name)
	}
	if parsed.Description != "A useful plain description." {
		t.Fatalf("Description = %q, want first paragraph fallback", parsed.Description)
	}
}

func TestScanAllIgnoresEscapedPluginPathsAndKeepsScanning(t *testing.T) {
	home := t.TempDir()
	writeFile(t, filepath.Join(home, "skills", "safe", "SKILL.md"), `---
name: safe
description: Safe local skill
---`)
	writeFile(t, filepath.Join(home, "plugins", "cache", "remote", "bad", "1.0.0", ".codex-plugin", "plugin.json"), `{
  "name": "bad",
  "version": "1.0.0",
  "skills": "..\\..\\..\\..\\outside",
  "apps": "./bad-app.json",
  "mcpServers": "./bad-mcp.json"
}`)
	writeFile(t, filepath.Join(home, "plugins", "cache", "remote", "bad", "1.0.0", "bad-app.json"), `{not json`)
	writeFile(t, filepath.Join(home, "plugins", "cache", "remote", "bad", "1.0.0", "bad-mcp.json"), `{not json`)

	result, err := ScanAll(context.Background(), Options{CodexHome: home})
	if err != nil {
		t.Fatalf("ScanAll should degrade bad optional plugin data, got error: %v", err)
	}
	assertAbility(t, result.Abilities, domain.KindSkill, "safe")
	assertAbility(t, result.Abilities, domain.KindPlugin, "bad")
	for _, ability := range result.Abilities {
		if ability.Name == "outside" {
			t.Fatalf("escaped plugin path was scanned: %+v", ability)
		}
	}
	if result.SourceNote == "" {
		t.Fatalf("SourceNote should mention skipped bad optional data")
	}
}

func assertAbility(t *testing.T, abilities []domain.Ability, kind domain.AbilityKind, name string) {
	t.Helper()
	for _, ability := range abilities {
		if ability.Kind == kind && ability.Name == name {
			return
		}
	}
	t.Fatalf("missing %s ability named %q in %#v", kind, name, abilities)
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
