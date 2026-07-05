package scanner

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"codex-atlas/internal/domain"
)

type Options struct {
	CodexHome string
}

type Result struct {
	CodexHome  string           `json:"codexHome"`
	ScannedAt  time.Time        `json:"scannedAt"`
	Abilities  []domain.Ability `json:"abilities"`
	SourceNote string           `json:"sourceNote,omitempty"`
}

type pluginManifest struct {
	Name        string          `json:"name"`
	Version     string          `json:"version"`
	Description string          `json:"description"`
	Skills      json.RawMessage `json:"skills"`
	Apps        json.RawMessage `json:"apps"`
	MCPServers  json.RawMessage `json:"mcpServers"`
	Interface   struct {
		DisplayName      string   `json:"displayName"`
		ShortDescription string   `json:"shortDescription"`
		LongDescription  string   `json:"longDescription"`
		Category         string   `json:"category"`
		Capabilities     []string `json:"capabilities"`
	} `json:"interface"`
}

// ScanAll 只读 Codex 目录，汇总 skills、plugins、tools 和 apps。
func ScanAll(ctx context.Context, opts Options) (Result, error) {
	home := opts.CodexHome
	if strings.TrimSpace(home) == "" {
		home = defaultCodexHome()
	}

	result := Result{CodexHome: home, ScannedAt: time.Now()}
	if _, err := os.Stat(home); err != nil {
		return result, fmt.Errorf("Codex 数据目录不可用: %w", err)
	}

	var abilities []domain.Ability
	localSkills, err := scanSkillRoot(ctx, filepath.Join(home, "skills"), "", "")
	if err != nil {
		return result, err
	}
	abilities = append(abilities, localSkills...)

	pluginAbilities, err := scanPlugins(ctx, filepath.Join(home, "plugins", "cache"))
	if err != nil {
		return result, err
	}
	abilities = append(abilities, pluginAbilities.Abilities...)
	result.SourceNote = strings.Join(pluginAbilities.Notes, "\n")

	sort.SliceStable(abilities, func(i, j int) bool {
		if abilities[i].Kind != abilities[j].Kind {
			return abilities[i].Kind < abilities[j].Kind
		}
		return strings.ToLower(abilities[i].Name) < strings.ToLower(abilities[j].Name)
	})
	result.Abilities = abilities
	return result, nil
}

func defaultCodexHome() string {
	if env := strings.TrimSpace(os.Getenv("CODEX_HOME")); env != "" {
		return env
	}
	return filepath.Join(os.Getenv("USERPROFILE"), ".codex")
}

func scanSkillRoot(ctx context.Context, root, pluginName, pluginVersion string) ([]domain.Ability, error) {
	if _, err := os.Stat(root); errors.Is(err, os.ErrNotExist) {
		return nil, nil
	}
	var abilities []domain.Ability
	err := filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if ctx.Err() != nil {
			return ctx.Err()
		}
		if d.IsDir() || !strings.EqualFold(d.Name(), "SKILL.md") {
			return nil
		}
		ability, err := ParseSkillFile(path, pluginName, pluginVersion)
		if err != nil {
			return err
		}
		abilities = append(abilities, ability)
		return nil
	})
	return abilities, err
}

// ParseSkillFile 解析 SKILL.md 的 front matter；缺字段时用目录名和首段 Markdown 回退。
func ParseSkillFile(path, pluginName, pluginVersion string) (domain.Ability, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return domain.Ability{}, err
	}
	text := string(content)
	meta, body := splitFrontMatter(text)
	name := strings.TrimSpace(meta["name"])
	if name == "" {
		name = filepath.Base(filepath.Dir(path))
	}
	description := strings.TrimSpace(meta["description"])
	if description == "" {
		description = firstParagraph(body)
	}

	displayName := name
	source := "local skill"
	if pluginName != "" {
		displayName = name
		name = pluginName + ":" + name
		source = "plugin skill"
	}

	return domain.Ability{
		ID:          string(domain.KindSkill) + ":" + name,
		Kind:        domain.KindSkill,
		Name:        name,
		DisplayName: displayName,
		Plugin:      pluginName,
		Version:     pluginVersion,
		Description: description,
		Summary:     description,
		Source:      source,
		SourcePath:  path,
		Directory:   filepath.Dir(path),
		Status:      "本地可见",
	}, nil
}

func splitFrontMatter(text string) (map[string]string, string) {
	meta := map[string]string{}
	normalized := strings.ReplaceAll(text, "\r\n", "\n")
	if !strings.HasPrefix(normalized, "---\n") {
		return meta, normalized
	}
	end := strings.Index(normalized[4:], "\n---")
	if end < 0 {
		return meta, normalized
	}
	block := normalized[4 : 4+end]
	body := normalized[4+end+4:]
	for _, line := range strings.Split(block, "\n") {
		key, value, ok := strings.Cut(line, ":")
		if !ok {
			continue
		}
		meta[strings.TrimSpace(key)] = strings.Trim(strings.TrimSpace(value), `"'`)
	}
	return meta, body
}

func firstParagraph(markdown string) string {
	for _, line := range strings.Split(strings.ReplaceAll(markdown, "\r\n", "\n"), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "---") {
			continue
		}
		return line
	}
	return ""
}

type pluginScanResult struct {
	Abilities []domain.Ability
	Notes     []string
}

func scanPlugins(ctx context.Context, cacheRoot string) (pluginScanResult, error) {
	if _, err := os.Stat(cacheRoot); errors.Is(err, os.ErrNotExist) {
		return pluginScanResult{}, nil
	}
	var result pluginScanResult
	err := filepath.WalkDir(cacheRoot, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if ctx.Err() != nil {
			return ctx.Err()
		}
		if d.IsDir() || !strings.EqualFold(d.Name(), "plugin.json") || filepath.Base(filepath.Dir(path)) != ".codex-plugin" {
			return nil
		}
		items, notes, err := parsePlugin(ctx, path)
		if err != nil {
			result.Notes = append(result.Notes, err.Error())
			return nil
		}
		result.Abilities = append(result.Abilities, items...)
		result.Notes = append(result.Notes, notes...)
		return nil
	})
	return result, err
}

func parsePlugin(ctx context.Context, manifestPath string) ([]domain.Ability, []string, error) {
	data, err := os.ReadFile(manifestPath)
	if err != nil {
		return nil, nil, err
	}
	var manifest pluginManifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		return nil, nil, fmt.Errorf("解析插件清单失败 %s: %w", manifestPath, err)
	}
	var notes []string
	pluginRoot := filepath.Dir(filepath.Dir(manifestPath))
	name := strings.TrimSpace(manifest.Name)
	if name == "" {
		name = filepath.Base(pluginRoot)
	}
	description := firstNonEmpty(manifest.Interface.ShortDescription, manifest.Description, manifest.Interface.LongDescription)
	plugin := domain.Ability{
		ID:          string(domain.KindPlugin) + ":" + name,
		Kind:        domain.KindPlugin,
		Name:        name,
		DisplayName: firstNonEmpty(manifest.Interface.DisplayName, name),
		Version:     manifest.Version,
		Description: description,
		Summary:     description,
		Tags:        manifest.Interface.Capabilities,
		Source:      "plugin manifest",
		SourcePath:  manifestPath,
		Directory:   pluginRoot,
		Status:      "本地可见",
	}
	abilities := []domain.Ability{plugin}

	skillRoots, pathNotes := manifestPaths(pluginRoot, manifest.Skills, "skills")
	notes = append(notes, pathNotes...)
	for _, skillRoot := range skillRoots {
		skills, err := scanSkillRoot(ctx, skillRoot, name, manifest.Version)
		if err != nil {
			notes = append(notes, fmt.Sprintf("跳过插件 skill 目录 %s: %v", skillRoot, err))
			continue
		}
		for _, skill := range skills {
			plugin.ChildIDs = append(plugin.ChildIDs, skill.ID)
			abilities = append(abilities, skill)
		}
	}
	abilities[0] = plugin

	appFiles, pathNotes := manifestPaths(pluginRoot, manifest.Apps, "")
	notes = append(notes, pathNotes...)
	for _, appFile := range appFiles {
		apps, err := parseApps(appFile, name)
		if err != nil {
			notes = append(notes, err.Error())
			continue
		}
		abilities = append(abilities, apps...)
	}
	mcpFiles, pathNotes := manifestPaths(pluginRoot, manifest.MCPServers, "")
	notes = append(notes, pathNotes...)
	for _, mcpFile := range mcpFiles {
		tools, err := parseTools(mcpFile, name)
		if err != nil {
			notes = append(notes, err.Error())
			continue
		}
		abilities = append(abilities, tools...)
	}
	return abilities, notes, nil
}

// manifestPaths 只接受插件目录内的相对路径，避免坏清单通过绝对路径或 .. 读取外部数据。
func manifestPaths(root string, raw json.RawMessage, fallback string) ([]string, []string) {
	var paths []string
	var notes []string
	if len(raw) > 0 && string(raw) != "null" {
		var single string
		if err := json.Unmarshal(raw, &single); err == nil && strings.TrimSpace(single) != "" {
			if path, err := safePluginPath(root, single); err == nil {
				paths = append(paths, path)
			} else {
				notes = append(notes, err.Error())
			}
		}
	}
	if len(paths) == 0 && fallback != "" {
		paths = append(paths, filepath.Join(root, fallback))
	}
	return paths, notes
}

func safePluginPath(root, value string) (string, error) {
	value = strings.TrimSpace(value)
	if filepath.IsAbs(value) {
		return "", fmt.Errorf("跳过插件目录外绝对路径: %s", value)
	}
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return "", err
	}
	joined, err := filepath.Abs(filepath.Clean(filepath.Join(rootAbs, value)))
	if err != nil {
		return "", err
	}
	rel, err := filepath.Rel(rootAbs, joined)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) || filepath.IsAbs(rel) {
		return "", fmt.Errorf("跳过插件目录外路径: %s", value)
	}
	return joined, nil
}

func parseApps(path, pluginName string) ([]domain.Ability, error) {
	data, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	var payload struct {
		Apps map[string]struct {
			ID       string `json:"id"`
			Optional bool   `json:"optional"`
			Required bool   `json:"required"`
		} `json:"apps"`
	}
	if err := json.Unmarshal(data, &payload); err != nil {
		return nil, fmt.Errorf("解析 apps 清单失败 %s: %w", path, err)
	}
	var abilities []domain.Ability
	for name, app := range payload.Apps {
		status := "可选"
		if app.Required {
			status = "必需"
		}
		abilities = append(abilities, domain.Ability{
			ID:          string(domain.KindApp) + ":" + pluginName + ":" + name,
			Kind:        domain.KindApp,
			Name:        name,
			DisplayName: name,
			Plugin:      pluginName,
			Description: app.ID,
			Summary:     app.ID,
			Source:      "plugin app manifest",
			SourcePath:  path,
			Directory:   filepath.Dir(path),
			Status:      status,
		})
	}
	return abilities, nil
}

func parseTools(path, pluginName string) ([]domain.Ability, error) {
	data, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	var payload struct {
		MCPServers map[string]struct {
			Title       string   `json:"title"`
			Description string   `json:"description"`
			Command     string   `json:"command"`
			Args        []string `json:"args"`
		} `json:"mcpServers"`
	}
	if err := json.Unmarshal(data, &payload); err != nil {
		return nil, fmt.Errorf("解析 MCP 清单失败 %s: %w", path, err)
	}
	var abilities []domain.Ability
	for name, tool := range payload.MCPServers {
		description := firstNonEmpty(tool.Description, tool.Command)
		abilities = append(abilities, domain.Ability{
			ID:          string(domain.KindTool) + ":" + pluginName + ":" + name,
			Kind:        domain.KindTool,
			Name:        name,
			DisplayName: firstNonEmpty(tool.Title, name),
			Plugin:      pluginName,
			Description: description,
			Summary:     description,
			Source:      "plugin mcp manifest",
			SourcePath:  path,
			Directory:   filepath.Dir(path),
			Status:      "本地清单可见",
		})
	}
	return abilities, nil
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}
