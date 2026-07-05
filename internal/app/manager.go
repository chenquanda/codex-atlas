package app

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"codex-atlas/internal/ai"
	"codex-atlas/internal/domain"
	"codex-atlas/internal/scanner"
	"codex-atlas/internal/stats"
	"codex-atlas/internal/store"
	"codex-atlas/internal/translate"
)

type Manager struct {
	mu              sync.Mutex
	ProjectRoot     string
	StorePath       string
	CachePath       string
	StatsPath       string
	TranslationPath string
	CodexHome       string
	State           domain.Store
	Scan            scanner.Result
	Stats           map[string]domain.Stats
	Translations    *translate.Cache
}

func NewManager(storePath, cachePath, translationPath, codexHome string) *Manager {
	if strings.TrimSpace(translationPath) == "" {
		translationPath = translationCachePath(cachePath)
	}
	return &Manager{
		ProjectRoot:     inferProjectRoot(storePath, cachePath, translationPath),
		StorePath:       storePath,
		CachePath:       cachePath,
		StatsPath:       statsCachePath(cachePath),
		TranslationPath: translationPath,
		CodexHome:       codexHome,
		State:           domain.NewStore(),
		Stats:           map[string]domain.Stats{},
	}
}

// Load 读取用户/AI 数据和最近扫描缓存，保证启动时无需立刻扫描也能显示旧列表。
func (m *Manager) Load() error {
	m.mu.Lock()
	defer m.mu.Unlock()
	jsonStore, err := store.NewBoundedJSONStore(m.ProjectRoot, m.StorePath)
	if err != nil {
		return err
	}
	state, err := jsonStore.Load()
	if err != nil {
		return err
	}
	m.State = state
	if m.Stats == nil {
		m.Stats = map[string]domain.Stats{}
	}
	scan, err := loadScanCache(m.CachePath)
	if err != nil {
		return err
	}
	m.Scan = scan
	stats, err := loadStatsCache(m.StatsPath)
	if err != nil {
		return err
	}
	m.Stats = stats
	if m.Stats == nil {
		m.Stats = map[string]domain.Stats{}
	}
	if err := m.ensureTranslationsLocked(); err != nil {
		return err
	}
	return nil
}

func (m *Manager) Save() error {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.saveLocked()
}

func (m *Manager) saveLocked() error {
	jsonStore, err := store.NewBoundedJSONStore(m.ProjectRoot, m.StorePath)
	if err != nil {
		return err
	}
	return jsonStore.Save(m.State)
}

func (m *Manager) UpdateUser(id string, mutate func(*domain.UserData)) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.State.Users == nil {
		m.State.Users = map[string]domain.UserData{}
	}
	user := m.State.Users[id]
	mutate(&user)
	user.UpdatedAt = time.Now()
	m.State.Users[id] = user
	return m.saveLocked()
}

// Abilities 返回 UI 列表数据。隐藏项默认不显示，但统计和用户数据仍保留。
func (m *Manager) Abilities(kind domain.AbilityKind, query string, includeHidden bool) []domain.Ability {
	m.mu.Lock()
	var out []domain.Ability
	for _, raw := range m.Scan.Abilities {
		if kind != "" && raw.Kind != kind {
			continue
		}
		merged := domain.MergeAbility(raw, m.State.Users[raw.ID], m.State.AI[raw.ID], m.Stats[raw.ID])
		if merged.User.Hidden && !includeHidden {
			continue
		}
		if !matchesQuery(merged, query) {
			continue
		}
		out = append(out, merged)
	}
	sort.SliceStable(out, func(i, j int) bool {
		if out[i].User.Favorite != out[j].User.Favorite {
			return out[i].User.Favorite
		}
		if out[i].User.SortWeight != out[j].User.SortWeight {
			return out[i].User.SortWeight > out[j].User.SortWeight
		}
		if out[i].Stats.UsageCount != out[j].Stats.UsageCount {
			return out[i].Stats.UsageCount > out[j].Stats.UsageCount
		}
		return strings.ToLower(out[i].DisplayName) < strings.ToLower(out[j].DisplayName)
	})
	m.mu.Unlock()

	for i := range out {
		if summary := m.TranslatedSummary(out[i]); summary != "" {
			out[i].Summary = summary
		}
	}
	return out
}

func (m *Manager) ensureTranslationsLocked() error {
	if m.Translations != nil {
		return nil
	}
	if strings.TrimSpace(m.TranslationPath) == "" {
		return errTranslationsUnavailable
	}
	cache, err := translate.LoadCache(m.ProjectRoot, m.TranslationPath)
	if err != nil {
		return err
	}
	m.Translations = cache
	return nil
}

func (m *Manager) RefreshScan(ctx context.Context) error {
	result, err := scanner.ScanAll(ctx, scanner.Options{CodexHome: m.CodexHome})
	if err != nil {
		return err
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	m.Scan = result
	return saveScanCache(m.ProjectRoot, m.CachePath, result)
}

func (m *Manager) RefreshStats() error {
	m.mu.Lock()
	home := m.CodexHome
	if strings.TrimSpace(home) == "" {
		home = m.Scan.CodexHome
	}
	abilities := append([]domain.Ability{}, m.Scan.Abilities...)
	m.mu.Unlock()
	result, err := stats.CollectSkillStats(filepath.Join(home, "sessions"), abilities)
	if err != nil {
		return err
	}
	m.mu.Lock()
	defer m.mu.Unlock()
	m.Stats = result
	return saveStatsCache(m.ProjectRoot, m.StatsPath, result)
}

func (m *Manager) ApplyAI(data map[string]domain.AIData) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.State.AI == nil {
		m.State.AI = map[string]domain.AIData{}
	}
	for id, item := range data {
		m.State.AI[id] = item
	}
	return m.saveLocked()
}

func (m *Manager) OrganizeAI(ctx context.Context, organizer ai.Organizer, kind domain.AbilityKind, query string, includeHidden bool) error {
	items := m.Abilities(kind, query, includeHidden)
	return m.OrganizeAIItems(ctx, organizer, items)
}

// OrganizeAIItems 接收调用方已经确定的列表快照。AI 调用可能较慢，因此不在执行期间持有 Manager 锁；
// 结果写回时再通过 ApplyAI 加锁保存，兼顾 UI 响应和数据一致性。
func (m *Manager) OrganizeAIItems(ctx context.Context, organizer ai.Organizer, items []domain.Ability) error {
	data, err := organizer.Organize(ctx, items)
	if err != nil {
		return err
	}
	return m.ApplyAI(data)
}

func matchesQuery(ability domain.Ability, query string) bool {
	query = strings.TrimSpace(strings.ToLower(query))
	if query == "" {
		return true
	}
	haystack := strings.ToLower(strings.Join([]string{
		ability.ID,
		ability.Name,
		ability.DisplayName,
		ability.Plugin,
		ability.Description,
		ability.Summary,
		ability.User.Note,
		strings.Join(ability.Tags, " "),
		strings.Join(ability.UseScenarios, " "),
		strings.Join(ability.AvoidScenarios, " "),
	}, " "))
	return strings.Contains(haystack, query)
}

func loadScanCache(path string) (scanner.Result, error) {
	if strings.TrimSpace(path) == "" {
		return scanner.Result{}, nil
	}
	data, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return scanner.Result{}, nil
	}
	if err != nil {
		return scanner.Result{}, err
	}
	var result scanner.Result
	if err := json.Unmarshal(data, &result); err != nil {
		return scanner.Result{}, err
	}
	return result, nil
}

func loadStatsCache(path string) (map[string]domain.Stats, error) {
	if strings.TrimSpace(path) == "" {
		return map[string]domain.Stats{}, nil
	}
	data, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return map[string]domain.Stats{}, nil
	}
	if err != nil {
		return map[string]domain.Stats{}, nil
	}
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		// 统计缓存是可丢弃数据，损坏时不阻止应用启动；用户可通过“刷新统计”重新生成。
		return map[string]domain.Stats{}, nil
	}
	result := map[string]domain.Stats{}
	for id, payload := range raw {
		var fields map[string]json.RawMessage
		if err := json.Unmarshal(payload, &fields); err != nil {
			continue
		}
		if _, ok := fields["usageCount"]; !ok {
			// 旧版缓存只有 sessionsUsed/mentions，口径已过期，避免把未知统计显示成 0。
			continue
		}
		var stat domain.Stats
		if err := json.Unmarshal(payload, &stat); err != nil {
			continue
		}
		stat.Known = true
		result[id] = stat
	}
	return result, nil
}

func saveStatsCache(projectRoot, path string, result map[string]domain.Stats) error {
	if strings.TrimSpace(path) == "" {
		return nil
	}
	if err := ensureProjectPath(projectRoot, path, "统计缓存"); err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		return err
	}
	tmp := path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

func saveScanCache(projectRoot, path string, result scanner.Result) error {
	if strings.TrimSpace(path) == "" {
		return nil
	}
	if err := ensureProjectPath(projectRoot, path, "扫描缓存"); err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0o644)
}

func ensureProjectPath(projectRoot, path, label string) error {
	if projectRoot == "" {
		return nil
	}
	root, err := filepath.Abs(projectRoot)
	if err != nil {
		return err
	}
	target, err := filepath.Abs(path)
	if err != nil {
		return err
	}
	rel, err := filepath.Rel(root, target)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) || filepath.IsAbs(rel) {
		return errors.New("拒绝写入项目目录外" + label)
	}
	return nil
}

func statsCachePath(cachePath string) string {
	if strings.TrimSpace(cachePath) == "" {
		return ""
	}
	return filepath.Join(filepath.Dir(cachePath), "stats-cache.json")
}

func inferProjectRoot(paths ...string) string {
	var roots []string
	for _, path := range paths {
		if strings.TrimSpace(path) == "" {
			continue
		}
		dir := filepath.Dir(path)
		if filepath.Base(dir) == "data" {
			dir = filepath.Dir(dir)
		}
		roots = append(roots, dir)
	}
	if len(roots) == 0 {
		return "."
	}
	root, err := filepath.Abs(roots[0])
	if err != nil {
		return roots[0]
	}
	for _, item := range roots[1:] {
		target, err := filepath.Abs(item)
		if err != nil {
			continue
		}
		for !isProjectRootAncestor(root, target) {
			parent := filepath.Dir(root)
			if parent == root {
				return root
			}
			root = parent
		}
	}
	return root
}

func isProjectRootAncestor(root, target string) bool {
	relativePath, err := filepath.Rel(root, target)
	if err != nil {
		return false
	}
	if relativePath == "." {
		return true
	}
	return relativePath != ".." && !strings.HasPrefix(relativePath, ".."+string(filepath.Separator)) && !filepath.IsAbs(relativePath)
}
