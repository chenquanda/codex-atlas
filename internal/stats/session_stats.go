package stats

import (
	"bufio"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"codex-atlas/internal/domain"
)

var explicitSkillTokenRegexp = regexp.MustCompile(`\$[A-Za-z0-9_-]+(?::[A-Za-z0-9_-]+)*`)

// CollectSkillStats 流式扫描 jsonl 会话文件，避免把大型历史一次性读进内存。
func CollectSkillStats(sessionsRoot string, abilities []domain.Ability) (map[string]domain.Stats, error) {
	result := map[string]domain.Stats{}
	index := buildMentionIndex(abilities)
	sourcePaths := buildSourcePathIndex(abilities)
	for _, ability := range abilities {
		if ability.Kind == domain.KindSkill {
			result[ability.ID] = domain.Stats{Known: true}
		}
	}
	if _, err := os.Stat(sessionsRoot); errors.Is(err, os.ErrNotExist) {
		return result, nil
	}

	err := filepath.WalkDir(sessionsRoot, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() || !strings.EqualFold(filepath.Ext(path), ".jsonl") {
			return nil
		}
		usagesByID, err := countFileUsages(path, index, sourcePaths)
		if err != nil {
			return err
		}
		info, _ := d.Info()
		for id, usage := range usagesByID {
			if usage.Count == 0 {
				continue
			}
			stat := result[id]
			stat.Known = true
			stat.UsageCount += usage.Count
			stat.SessionsUsed++
			stat.Mentions += usage.Count
			if usage.LastUsedAt.After(stat.LastUsedAt) {
				stat.LastUsedAt = usage.LastUsedAt
			} else if usage.LastUsedAt.IsZero() && info != nil && info.ModTime().After(stat.LastUsedAt) {
				stat.LastUsedAt = info.ModTime()
			}
			result[id] = stat
		}
		return nil
	})
	return result, err
}

type mentionIndex struct {
	tokenToIDs map[string][]string
}

func (m mentionIndex) lookup(token string) []string {
	token = strings.TrimPrefix(strings.ToLower(strings.TrimSpace(token)), "$")
	return m.tokenToIDs[token]
}

func buildMentionIndex(abilities []domain.Ability) mentionIndex {
	shortCount := map[string]int{}
	for _, ability := range abilities {
		if ability.Kind != domain.KindSkill || strings.TrimSpace(ability.Name) == "" {
			continue
		}
		shortCount[strings.ToLower(shortSkillName(ability.Name))]++
	}
	index := mentionIndex{tokenToIDs: map[string][]string{}}
	for _, ability := range abilities {
		if ability.Kind != domain.KindSkill || strings.TrimSpace(ability.Name) == "" {
			continue
		}
		full := strings.ToLower(ability.Name)
		index.tokenToIDs[full] = appendUnique(index.tokenToIDs[full], ability.ID)
		short := strings.ToLower(shortSkillName(ability.Name))
		// 完整名永远可匹配；短名只有唯一且不会覆盖本地同名 skill 时才作为 fallback。
		if short != full && shortCount[short] == 1 {
			if _, exists := index.tokenToIDs[short]; !exists {
				index.tokenToIDs[short] = appendUnique(index.tokenToIDs[short], ability.ID)
			}
		}
	}
	return index
}

func shortSkillName(name string) string {
	if strings.Contains(name, ":") {
		parts := strings.Split(name, ":")
		return parts[len(parts)-1]
	}
	return name
}

func appendUnique(values []string, value string) []string {
	for _, existing := range values {
		if existing == value {
			return values
		}
	}
	return append(values, value)
}

type sourcePathRef struct {
	ID   string
	Path string
}

func buildSourcePathIndex(abilities []domain.Ability) []sourcePathRef {
	var refs []sourcePathRef
	for _, ability := range abilities {
		if ability.Kind != domain.KindSkill || strings.TrimSpace(ability.SourcePath) == "" {
			continue
		}
		refs = append(refs, sourcePathRef{ID: ability.ID, Path: normalizePathText(ability.SourcePath)})
	}
	return refs
}

type fileUsage struct {
	Count      int
	LastUsedAt time.Time
}

func countFileUsages(path string, index mentionIndex, sourcePaths []sourcePathRef) (map[string]fileUsage, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	counts := map[string]fileUsage{}
	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 64*1024), 64*1024*1024)
	for scanner.Scan() {
		line := scanner.Text()
		event, ok := parseSessionEvent(line)
		if !ok {
			continue
		}
		if event.Payload.Type == "message" && event.Payload.Role == "user" {
			// 用户显式写出的 $skill 才算一次真实调用；普通讨论中的 skill 名称不计入。
			for _, text := range event.contentTexts() {
				for _, token := range explicitSkillTokenRegexp.FindAllString(text, -1) {
					for _, id := range index.lookup(token) {
						addUsage(counts, id, 1, event.Time())
					}
				}
			}
			continue
		}
		if event.Payload.Type == "function_call" {
			arguments := normalizePathText(event.Payload.Arguments)
			for _, ref := range sourcePaths {
				if ref.Path != "" && strings.Contains(arguments, ref.Path) {
					addUsage(counts, ref.ID, 1, event.Time())
				}
			}
		}
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	return counts, nil
}

func addUsage(counts map[string]fileUsage, id string, count int, usedAt time.Time) {
	usage := counts[id]
	usage.Count += count
	if usedAt.After(usage.LastUsedAt) {
		usage.LastUsedAt = usedAt
	}
	counts[id] = usage
}

type sessionEvent struct {
	Timestamp string `json:"timestamp"`
	Type      string `json:"type"`
	Payload   struct {
		Type      string          `json:"type"`
		Role      string          `json:"role"`
		Content   json.RawMessage `json:"content"`
		Arguments string          `json:"arguments"`
	} `json:"payload"`
}

func parseSessionEvent(line string) (sessionEvent, bool) {
	var event sessionEvent
	if err := json.Unmarshal([]byte(line), &event); err != nil {
		return sessionEvent{}, false
	}
	return event, true
}

func (e sessionEvent) Time() time.Time {
	t, err := time.Parse(time.RFC3339Nano, strings.TrimSpace(e.Timestamp))
	if err != nil {
		return time.Time{}
	}
	return t
}

func (e sessionEvent) contentTexts() []string {
	if len(e.Payload.Content) == 0 {
		return nil
	}
	var text string
	if err := json.Unmarshal(e.Payload.Content, &text); err == nil {
		return []string{text}
	}
	var parts []struct {
		Text string `json:"text"`
	}
	if err := json.Unmarshal(e.Payload.Content, &parts); err == nil {
		var out []string
		for _, part := range parts {
			if strings.TrimSpace(part.Text) != "" {
				out = append(out, part.Text)
			}
		}
		return out
	}
	return nil
}

func normalizePathText(path string) string {
	path = strings.ReplaceAll(path, "\\", "/")
	path = strings.ReplaceAll(path, "\\/", "/")
	return strings.ToLower(filepath.Clean(path))
}
