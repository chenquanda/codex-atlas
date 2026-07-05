package ai

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"codex-atlas/internal/domain"
)

type Organizer struct {
	Command      string
	Args         []string
	WorkDir      string
	Timeout      time.Duration
	OutputPath   string
	PromptPrefix string
}

type organizeRequest struct {
	Abilities []domain.Ability `json:"abilities"`
}

type organizeResponse struct {
	Items []struct {
		ID             string                `json:"id"`
		Summary        string                `json:"summary"`
		Tags           []string              `json:"tags"`
		Templates      []domain.CallTemplate `json:"templates"`
		UseScenarios   []string              `json:"useScenarios"`
		AvoidScenarios []string              `json:"avoidScenarios"`
		Similar        []string              `json:"similar"`
	} `json:"items"`
}

// Organize 通过本机 codex 命令做一次性整理，输出只合并到 AIData 层。
func (o Organizer) Organize(ctx context.Context, abilities []domain.Ability) (map[string]domain.AIData, error) {
	command := strings.TrimSpace(o.Command)
	if command == "" {
		command = "codex"
	}
	workDir := strings.TrimSpace(o.WorkDir)
	if workDir == "" {
		workDir = "."
	}
	workAbs, err := filepath.Abs(workDir)
	if err != nil {
		return nil, err
	}
	timeout := o.Timeout
	if timeout == 0 {
		timeout = 90 * time.Second
	}
	tmpDir := filepath.Join(workAbs, ".tmp")
	if err := os.MkdirAll(tmpDir, 0o755); err != nil {
		return nil, err
	}
	if err := ensureRealPathInside(workAbs, tmpDir, "AI 整理临时目录"); err != nil {
		return nil, err
	}
	outputPath, _, err := resolveTmpOutputPath(workAbs, tmpDir, o.OutputPath, "ai-output.json", "AI 整理输出路径")
	if err != nil {
		return nil, err
	}
	outputDir := filepath.Dir(outputPath)
	if err := ensureRealPathInside(tmpDir, outputDir, "AI 整理输出目录"); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(outputDir, 0o755); err != nil {
		return nil, err
	}
	if err := ensureRealPathInside(tmpDir, outputDir, "AI 整理输出目录"); err != nil {
		return nil, err
	}
	inputPath := filepath.Join(tmpDir, "ai-input.json")
	input, err := json.MarshalIndent(organizeRequest{Abilities: abilities}, "", "  ")
	if err != nil {
		return nil, err
	}
	if err := os.WriteFile(inputPath, input, 0o644); err != nil {
		return nil, err
	}

	runCtx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()
	args := append([]string{}, o.Args...)
	if command == "codex" && len(args) == 0 {
		args = []string{"exec", "--skip-git-repo-check", "--ephemeral", "-o", outputPath, "-"}
	}
	cmd := exec.CommandContext(runCtx, command, args...)
	cmd.Dir = workAbs
	cmd.Stdin = bytes.NewReader(buildPrompt(o.PromptPrefix, input))
	cmd.Env = boundedEnv(os.Environ(), tmpDir)
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		if errors.Is(runCtx.Err(), context.DeadlineExceeded) {
			return nil, fmt.Errorf("AI 整理超时: %w", runCtx.Err())
		}
		return nil, fmt.Errorf("AI 整理命令失败: %w; stderr=%s stdout=%s", err, strings.TrimSpace(stderr.String()), strings.TrimSpace(stdout.String()))
	}

	output := stdout.Bytes()
	if fileOutput, err := os.ReadFile(outputPath); err == nil && len(bytes.TrimSpace(fileOutput)) > 0 {
		output = fileOutput
	} else {
		_ = os.WriteFile(outputPath, stdout.Bytes(), 0o644)
	}
	var response organizeResponse
	if err := json.Unmarshal(extractJSON(output), &response); err != nil {
		return nil, fmt.Errorf("AI 整理输出不是有效 JSON: %w; stdout=%s", err, strings.TrimSpace(string(output)))
	}
	if response.Items == nil {
		return nil, errors.New("AI 整理输出缺少 items 字段")
	}
	allowed := map[string]bool{}
	for _, ability := range abilities {
		allowed[ability.ID] = true
	}
	out := map[string]domain.AIData{}
	now := time.Now()
	for _, item := range response.Items {
		if strings.TrimSpace(item.ID) == "" {
			continue
		}
		if !allowed[item.ID] {
			return nil, fmt.Errorf("AI 整理返回了输入范围外的能力 ID: %s", item.ID)
		}
		out[item.ID] = domain.AIData{
			Summary:        strings.TrimSpace(item.Summary),
			Tags:           item.Tags,
			Templates:      item.Templates,
			UseScenarios:   item.UseScenarios,
			AvoidScenarios: item.AvoidScenarios,
			Similar:        item.Similar,
			UpdatedAt:      now,
		}
	}
	return out, nil
}

// boundedEnv 把外部命令的临时目录约束到项目内，减少命令运行时向系统临时目录落文件。
func boundedEnv(base []string, tmpDir string) []string {
	filtered := make([]string, 0, len(base)+3)
	for _, item := range base {
		upper := strings.ToUpper(item)
		if strings.HasPrefix(upper, "TMP=") || strings.HasPrefix(upper, "TEMP=") {
			continue
		}
		filtered = append(filtered, item)
	}
	filtered = append(filtered, "TMP="+tmpDir, "TEMP="+tmpDir, "NO_COLOR=1")
	return filtered
}

func buildPrompt(prefix string, input []byte) []byte {
	if strings.TrimSpace(prefix) == "" {
		prefix = "你是 Codex Atlas 的离线整理器。请只输出 JSON，不要输出 Markdown。输出格式必须是 {\"items\":[{\"id\":\"...\",\"summary\":\"...\",\"tags\":[\"...\"],\"templates\":[{\"title\":\"...\",\"text\":\"...\"}],\"useScenarios\":[\"...\"],\"avoidScenarios\":[\"...\"],\"similar\":[\"...\"]}]}。只整理输入里的能力，不要读取或修改任何文件。"
	}
	var buf bytes.Buffer
	buf.WriteString(prefix)
	buf.WriteString("\n\n输入 JSON:\n")
	buf.Write(input)
	return buf.Bytes()
}

func extractJSON(output []byte) []byte {
	text := strings.TrimSpace(string(output))
	if strings.HasPrefix(text, "```") {
		text = strings.TrimPrefix(text, "```json")
		text = strings.TrimPrefix(text, "```")
		text = strings.TrimSuffix(text, "```")
	}
	return []byte(strings.TrimSpace(text))
}
