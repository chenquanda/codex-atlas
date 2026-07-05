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
	"sync"
	"time"
)

type Translator struct {
	Command      string
	Args         []string
	WorkDir      string
	Timeout      time.Duration
	OutputPath   string
	PromptPrefix string
}

type TranslateRequest struct {
	SkillName string `json:"skillName"`
	Path      string `json:"path"`
	Text      string `json:"text"`
}

type TranslateResponse struct {
	TranslatedText string `json:"translatedText"`
	Summary        string `json:"summary"`
}

var defaultTranslationOutputMu sync.Mutex

func (t Translator) Translate(ctx context.Context, request TranslateRequest) (TranslateResponse, error) {
	command := strings.TrimSpace(t.Command)
	if command == "" {
		command = "codex"
	}
	workDir := strings.TrimSpace(t.WorkDir)
	if workDir == "" {
		workDir = "."
	}
	workAbs, err := filepath.Abs(workDir)
	if err != nil {
		return TranslateResponse{}, err
	}
	timeout := t.Timeout
	if timeout == 0 {
		timeout = 90 * time.Second
	}
	tmpDir := filepath.Join(workAbs, ".tmp")
	if err := os.MkdirAll(tmpDir, 0o755); err != nil {
		return TranslateResponse{}, err
	}
	if err := ensureRealPathInside(workAbs, tmpDir, "AI 翻译临时目录"); err != nil {
		return TranslateResponse{}, err
	}
	outputPath, isDefaultOutput, err := resolveTmpOutputPath(workAbs, tmpDir, t.OutputPath, "translation-output.json", "AI 翻译输出路径")
	if err != nil {
		return TranslateResponse{}, err
	}
	if isDefaultOutput {
		// 默认 output path 是固定文件；同进程并发翻译必须串行化，避免互相删除或读取对方输出。
		defaultTranslationOutputMu.Lock()
		defer defaultTranslationOutputMu.Unlock()
	}
	outputDir := filepath.Dir(outputPath)
	if err := ensureRealPathInside(tmpDir, outputDir, "AI 翻译输出目录"); err != nil {
		return TranslateResponse{}, err
	}
	if err := os.MkdirAll(outputDir, 0o755); err != nil {
		return TranslateResponse{}, err
	}
	if err := ensureRealPathInside(tmpDir, outputDir, "AI 翻译输出目录"); err != nil {
		return TranslateResponse{}, err
	}
	// 输出文件边界：命令前清掉旧文件，避免读取上一次翻译留下的 JSON。
	if err := os.Remove(outputPath); err != nil && !errors.Is(err, os.ErrNotExist) {
		return TranslateResponse{}, err
	}

	input, err := json.MarshalIndent(request, "", "  ")
	if err != nil {
		return TranslateResponse{}, err
	}

	runCtx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()
	args := append([]string{}, t.Args...)
	if command == "codex" && len(args) == 0 {
		args = []string{"exec", "--skip-git-repo-check", "--ephemeral", "-o", outputPath, "-"}
	}
	cmd := exec.CommandContext(runCtx, command, args...)
	cmd.Dir = workAbs
	cmd.Stdin = bytes.NewReader(buildTranslationPrompt(t.PromptPrefix, input))
	// 外部命令边界：只把临时目录暴露到项目 .tmp 下，避免翻译过程向系统临时目录散落文件。
	cmd.Env = boundedEnv(os.Environ(), tmpDir)
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		if errors.Is(runCtx.Err(), context.DeadlineExceeded) {
			return TranslateResponse{}, fmt.Errorf("AI 翻译超时: %w", runCtx.Err())
		}
		return TranslateResponse{}, fmt.Errorf("AI 翻译命令失败: %w; stderr=%s stdout=%s", err, strings.TrimSpace(stderr.String()), strings.TrimSpace(stdout.String()))
	}

	output := stdout.Bytes()
	if fileOutput, err := os.ReadFile(outputPath); err == nil && len(bytes.TrimSpace(fileOutput)) > 0 {
		output = fileOutput
	} else {
		_ = os.WriteFile(outputPath, stdout.Bytes(), 0o644)
	}
	var response TranslateResponse
	if err := json.Unmarshal(extractJSON(output), &response); err != nil {
		return TranslateResponse{}, fmt.Errorf("AI 翻译输出不是有效 JSON: %w; stdout=%s", err, strings.TrimSpace(string(output)))
	}
	response.TranslatedText = strings.TrimSpace(response.TranslatedText)
	response.Summary = strings.TrimSpace(response.Summary)
	if response.TranslatedText == "" {
		return TranslateResponse{}, errors.New("AI 翻译输出缺少 translatedText 字段")
	}
	return response, nil
}

func ensureRealPathInside(root, target, label string) error {
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return err
	}
	targetAbs, err := filepath.Abs(target)
	if err != nil {
		return err
	}
	if !isPathInside(rootAbs, targetAbs) {
		return fmt.Errorf("%s必须位于项目目录内: %s", label, rootAbs)
	}
	rootEval, err := filepath.EvalSymlinks(rootAbs)
	if err != nil {
		return err
	}
	existingPath, err := nearestExistingPath(targetAbs)
	if err != nil {
		return err
	}
	existingEval, err := filepath.EvalSymlinks(existingPath)
	if err != nil {
		return err
	}
	// 词法路径只能挡住 ..；这里再看真实落点，防止 .tmp 或子目录是指向项目外的 junction/symlink。
	if !isPathInside(rootEval, existingEval) {
		return fmt.Errorf("%s真实路径必须位于项目目录内: %s", label, rootEval)
	}
	return nil
}

func nearestExistingPath(path string) (string, error) {
	current := path
	for {
		if _, err := os.Lstat(current); err == nil {
			return current, nil
		} else if !errors.Is(err, os.ErrNotExist) {
			return "", err
		}
		parent := filepath.Dir(current)
		if parent == current {
			return "", os.ErrNotExist
		}
		current = parent
	}
}

func resolveTmpOutputPath(workDir, tmpDir, configuredPath, defaultName, label string) (string, bool, error) {
	tmpAbs, err := filepath.Abs(tmpDir)
	if err != nil {
		return "", false, err
	}
	outputPath := strings.TrimSpace(configuredPath)
	if outputPath == "" {
		return filepath.Join(tmpAbs, defaultName), true, nil
	}
	if !filepath.IsAbs(outputPath) {
		outputPath = filepath.Join(workDir, outputPath)
	}
	outputAbs, err := filepath.Abs(outputPath)
	if err != nil {
		return "", false, err
	}
	if !isPathInside(tmpAbs, outputAbs) {
		return "", false, fmt.Errorf("%s必须位于项目临时目录内: %s", label, tmpAbs)
	}
	return outputAbs, false, nil
}

func isPathInside(root, target string) bool {
	rel, err := filepath.Rel(root, target)
	if err != nil {
		return false
	}
	return rel == "." || (rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator)) && !filepath.IsAbs(rel))
}

func buildTranslationPrompt(prefix string, input []byte) []byte {
	if strings.TrimSpace(prefix) == "" {
		prefix = "你是 Codex Atlas 的离线 skill 文档翻译器。请只输出 JSON，不要输出 Markdown。输出格式必须是 {\"translatedText\":\"...\",\"summary\":\"...\"}。translatedText 使用简体中文完整翻译输入文本，summary 用一句简体中文概括文档用途。不要读取或修改任何文件。"
	}
	var buf bytes.Buffer
	buf.WriteString(prefix)
	buf.WriteString("\n\n输入 JSON:\n")
	buf.Write(input)
	return buf.Bytes()
}
