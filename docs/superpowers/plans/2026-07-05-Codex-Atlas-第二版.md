# Codex Atlas 第二版 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 Codex Atlas 第二版做成更宽、更可读的能力索引工具，并新增 Skill 文件夹阅读、按需翻译、译文缓存和中文优先展示。

**Architecture:** 保持 Go + Walk 原生桌面架构，不引入 Electron/WebView。新增 `internal/skilldoc` 负责只读枚举和读取 skill 文件夹，新增 `internal/translate` 负责语言判断和 JSON 缓存，新增 `internal/ai/translator.go` 复用本机 `codex exec --ephemeral` 做一次性翻译，UI 层新增独立 Skill 详情窗口。

**Tech Stack:** Go 1.26、`github.com/lxn/walk`、Windows 原生控件、项目内 JSON 缓存、项目内 `.tmp` 临时文件、本机 `codex` CLI。

---

## Scope

本计划只覆盖第二版需求：

- 主窗口扩大并重排控件，不再是极窄侧栏。
- 点击 Skill 打开独立详情窗口。
- 详情窗口展示 skill 文件夹内可预览文件，点击文件显示内容。
- 英文内容可调用 `codex exec --ephemeral` 翻译。
- 翻译结果写入 `data/translation-cache.json`，按 `skillID + relativePath + contentHash` 命中。
- 有缓存时默认显示中文译文。
- 中文文件不显示翻译按钮。
- 主列表和摘要区域优先显示中文翻译或中文整理内容。

不做这些内容：

- 不做云同步、账号、团队共享。
- 不修改 `.codex` 目录内任何文件。
- 不把主程序改成 Electron 或 WebView。
- 不自动后台全量翻译所有 skill。

## File Structure

### New Files

- `internal/skilldoc/reader.go`  
  只读列出 skill 目录文件、读取文本文件、拒绝越界路径、识别过大文件和二进制文件。

- `internal/skilldoc/reader_test.go`  
  覆盖目录枚举、相对路径读取、路径穿越拒绝、二进制拒绝、文件大小限制。

- `internal/translate/language.go`  
  轻量语言判断：中文比例达到阈值则视为中文，否则英文或混合。用于决定是否显示翻译按钮。

- `internal/translate/language_test.go`  
  覆盖中文 skill、英文 skill、混合文本、短文本。

- `internal/translate/cache.go`  
  `translation-cache.json` 的读写、缓存 key、内容 hash、缓存命中判断。

- `internal/translate/cache_test.go`  
  覆盖缓存保存/重载、hash 变化失效、旧缓存损坏时不阻止启动、写入边界必须在项目内。

- `internal/ai/translator.go`  
  封装单文件翻译调用，输入一段文本，输出中文译文和短摘要。命令默认使用 `codex exec --skip-git-repo-check --ephemeral -o <output> -`。

- `internal/ai/translator_test.go`  
  使用 fake command 覆盖成功 JSON、失败命令、坏 JSON、超时。

- `internal/app/skill_detail.go`  
  Manager 层组合 skilldoc、translate cache 和 ai translator，给 UI 提供 `ListSkillFiles`、`ReadSkillFile`、`TranslateSkillFile`、`TranslatedSummary`。

- `internal/app/skill_detail_test.go`  
  覆盖主流程：英文无缓存默认原文、有缓存默认译文、中文文件不需要翻译、翻译后缓存持久化。

- `internal/ui/skill_detail_windows.go`  
  Windows 详情弹窗：左侧文件列表，右侧原文/译文阅读，工具栏按钮状态。

### Modified Files

- `internal/config/paths.go`  
  `Paths` 增加 `TranslationPath string`，默认 `data/translation-cache.json`。

- `cmd/codex-atlas/main.go`  
  `app.NewManager` 增加翻译缓存路径参数。

- `internal/app/manager.go`  
  Manager 增加 `TranslationPath`、加载翻译缓存、提供摘要中文优先合并。

- `internal/domain/types.go`  
  如需要，增加 `TranslationState` 或最小展示字段；优先避免污染核心 `Ability`，能放 Manager/UI 计算就不加。

- `internal/ui/window_windows.go`  
  主窗口尺寸、布局、按钮、打开详情入口、双击行为、摘要优先中文。

- `internal/ui/list_model_windows.go`  
  列表显示改为第二版信息结构。若 Walk `ListBox` 无法稳定展示多列，则改用 `TableView`。

- `scripts/build.ps1`  
  构建时把 `data/translation-cache.json` 复制到 `dist/data/`，不存在时跳过。

- `README.md`、`docs/使用说明.md`、`docs/方案设计.md`、`docs/实施计划.md`、`docs/开发记录.md`  
  更新第二版功能、缓存文件、翻译行为、验证记录。

---

## Task 1: Skill 文件夹只读阅读模块

**Files:**
- Create: `internal/skilldoc/reader.go`
- Create: `internal/skilldoc/reader_test.go`

- [ ] **Step 1: 写失败测试，覆盖目录枚举和相对路径读取**

Create `internal/skilldoc/reader_test.go` with:

```go
package skilldoc

import (
	"os"
	"path/filepath"
	"testing"
)

func TestReaderListsAndReadsSkillFiles(t *testing.T) {
	dir := t.TempDir()
	mustWrite(t, filepath.Join(dir, "SKILL.md"), "# Skill\n\nEnglish body")
	mustWrite(t, filepath.Join(dir, "references", "workflow.md"), "# Workflow\n\nRead context")
	mustWrite(t, filepath.Join(dir, "scripts", "prepare.js"), "console.log('ok')")

	reader := Reader{MaxBytes: 64 * 1024}
	files, err := reader.List(dir)
	if err != nil {
		t.Fatalf("List returned error: %v", err)
	}
	want := []string{"SKILL.md", "references/workflow.md", "scripts/prepare.js"}
	if got := relativePaths(files); !equalStrings(got, want) {
		t.Fatalf("files = %#v, want %#v", got, want)
	}

	doc, err := reader.Read(dir, "references/workflow.md")
	if err != nil {
		t.Fatalf("Read returned error: %v", err)
	}
	if doc.RelativePath != "references/workflow.md" || doc.Text != "# Workflow\n\nRead context" {
		t.Fatalf("doc = %+v", doc)
	}
}

func mustWrite(t *testing.T, path string, text string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("MkdirAll: %v", err)
	}
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
}
```

- [ ] **Step 2: 写失败测试，覆盖安全边界和不可预览文件**

Append:

```go
func TestReaderRejectsTraversalLargeAndBinaryFiles(t *testing.T) {
	dir := t.TempDir()
	mustWrite(t, filepath.Join(dir, "SKILL.md"), "ok")
	mustWrite(t, filepath.Join(dir, "large.md"), strings.Repeat("a", 32))
	if err := os.WriteFile(filepath.Join(dir, "binary.bin"), []byte{0x00, 0x01, 0x02}, 0o644); err != nil {
		t.Fatalf("WriteFile binary: %v", err)
	}

	reader := Reader{MaxBytes: 8}
	if _, err := reader.Read(dir, "../outside.md"); err == nil {
		t.Fatalf("Read should reject traversal")
	}
	if _, err := reader.Read(dir, "large.md"); !errors.Is(err, ErrFileTooLarge) {
		t.Fatalf("large error = %v, want ErrFileTooLarge", err)
	}
	if _, err := reader.Read(dir, "binary.bin"); !errors.Is(err, ErrBinaryFile) {
		t.Fatalf("binary error = %v, want ErrBinaryFile", err)
	}
}
```

The imports must include `errors` and `strings`.

- [ ] **Step 3: 运行测试确认失败**

Run:

```powershell
$env:GOCACHE = (Resolve-Path .cache\gobuild).Path
$env:GOMODCACHE = (Resolve-Path .cache\gomod).Path
go test ./internal/skilldoc
```

Expected: FAIL because package or types are not implemented.

- [ ] **Step 4: 实现 `internal/skilldoc/reader.go`**

```go
package skilldoc

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

var (
	ErrPathOutside = errors.New("文件路径不在 skill 目录内")
	ErrFileTooLarge = errors.New("文件过大，无法预览")
	ErrBinaryFile = errors.New("二进制文件无法预览")
)

type Reader struct {
	MaxBytes int64
}

type FileInfo struct {
	RelativePath string
	Name         string
	Ext          string
	Size         int64
}

type Document struct {
	RelativePath string
	AbsolutePath string
	Text         string
	Size         int64
}

func (r Reader) List(root string) ([]FileInfo, error) {
	var out []FileInfo
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			return nil
		}
		rel, err := safeRel(root, path)
		if err != nil {
			return err
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		out = append(out, FileInfo{
			RelativePath: filepath.ToSlash(rel),
			Name: entry.Name(),
			Ext: strings.TrimPrefix(strings.ToLower(filepath.Ext(entry.Name())), "."),
			Size: info.Size(),
		})
		return nil
	})
	sort.SliceStable(out, func(i, j int) bool {
		if out[i].RelativePath == "SKILL.md" {
			return true
		}
		if out[j].RelativePath == "SKILL.md" {
			return false
		}
		return out[i].RelativePath < out[j].RelativePath
	})
	return out, err
}

func (r Reader) Read(root, relativePath string) (Document, error) {
	maxBytes := r.MaxBytes
	if maxBytes == 0 {
		maxBytes = 256 * 1024
	}
	target := filepath.Join(root, filepath.FromSlash(relativePath))
	rel, err := safeRel(root, target)
	if err != nil {
		return Document{}, err
	}
	info, err := os.Stat(target)
	if err != nil {
		return Document{}, err
	}
	if info.Size() > maxBytes {
		return Document{}, ErrFileTooLarge
	}
	data, err := os.ReadFile(target)
	if err != nil {
		return Document{}, err
	}
	if bytes.IndexByte(data, 0) >= 0 {
		return Document{}, ErrBinaryFile
	}
	return Document{
		RelativePath: filepath.ToSlash(rel),
		AbsolutePath: target,
		Text: string(data),
		Size: info.Size(),
	}, nil
}

func safeRel(root, target string) (string, error) {
	absRoot, err := filepath.Abs(root)
	if err != nil {
		return "", err
	}
	absTarget, err := filepath.Abs(target)
	if err != nil {
		return "", err
	}
	rel, err := filepath.Rel(absRoot, absTarget)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) || filepath.IsAbs(rel) {
		return "", ErrPathOutside
	}
	return rel, nil
}
```

- [ ] **Step 5: 补齐测试 helper 并跑通过**

Add helpers to the test file:

```go
func relativePaths(files []FileInfo) []string {
	out := make([]string, 0, len(files))
	for _, file := range files {
		out = append(out, file.RelativePath)
	}
	return out
}

func equalStrings(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
```

Run:

```powershell
go test ./internal/skilldoc
```

Expected: PASS.

---

## Task 2: 语言判断与翻译缓存

**Files:**
- Create: `internal/translate/language.go`
- Create: `internal/translate/language_test.go`
- Create: `internal/translate/cache.go`
- Create: `internal/translate/cache_test.go`
- Modify: `internal/config/paths.go`
- Modify: `cmd/codex-atlas/main.go`

- [ ] **Step 1: 写语言判断测试**

Create `internal/translate/language_test.go`:

```go
package translate

import "testing"

func TestDetectLanguage(t *testing.T) {
	tests := []struct {
		name string
		text string
		want Language
	}{
		{"chinese", "这是一个用于调试和验证的 skill。", LanguageChinese},
		{"english", "Use this before implementing a feature or changing behavior.", LanguageEnglish},
		{"mixed-chinese", "使用 $auto-git-sync 完成 commit and push。", LanguageChinese},
		{"unknown-short", "$teach", LanguageUnknown},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := DetectLanguage(tt.text); got != tt.want {
				t.Fatalf("DetectLanguage = %s, want %s", got, tt.want)
			}
		})
	}
}
```

- [ ] **Step 2: 实现语言判断**

Create `internal/translate/language.go`:

```go
package translate

type Language string

const (
	LanguageUnknown Language = "unknown"
	LanguageChinese Language = "zh"
	LanguageEnglish Language = "en"
)

func DetectLanguage(text string) Language {
	var chinese int
	var latin int
	for _, r := range text {
		switch {
		case r >= '\u4e00' && r <= '\u9fff':
			chinese++
		case (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z'):
			latin++
		}
	}
	if chinese+latin < 12 {
		return LanguageUnknown
	}
	if chinese >= 4 && chinese*100/(chinese+latin) >= 18 {
		return LanguageChinese
	}
	return LanguageEnglish
}
```

- [ ] **Step 3: 写翻译缓存测试**

Create `internal/translate/cache_test.go`:

```go
package translate

import (
	"path/filepath"
	"testing"
	"time"
)

func TestCacheStoresAndInvalidatesByHash(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "data", "translation-cache.json")
	cache, err := LoadCache(dir, path)
	if err != nil {
		t.Fatalf("LoadCache returned error: %v", err)
	}
	entry := Entry{
		SkillID: "skill:brainstorming",
		RelativePath: "SKILL.md",
		ContentHash: HashText("hello"),
		SourceLanguage: LanguageEnglish,
		TranslatedText: "你好",
		Summary: "中文摘要",
		UpdatedAt: time.Now(),
	}
	cache.Set(entry)
	if err := cache.Save(); err != nil {
		t.Fatalf("Save returned error: %v", err)
	}

	reloaded, err := LoadCache(dir, path)
	if err != nil {
		t.Fatalf("reload returned error: %v", err)
	}
	got, ok := reloaded.Get("skill:brainstorming", "SKILL.md", "hello")
	if !ok || got.TranslatedText != "你好" || got.Summary != "中文摘要" {
		t.Fatalf("cache miss or bad entry: %+v ok=%v", got, ok)
	}
	if _, ok := reloaded.Get("skill:brainstorming", "SKILL.md", "changed"); ok {
		t.Fatalf("cache should miss after content hash changes")
	}
}

func TestCacheRejectsOutsideProjectPath(t *testing.T) {
	dir := t.TempDir()
	if _, err := LoadCache(dir, filepath.Join(filepath.Dir(dir), "translation-cache.json")); err == nil {
		t.Fatalf("LoadCache should reject paths outside project root")
	}
}
```

- [ ] **Step 4: 实现翻译缓存**

Create `internal/translate/cache.go` with:

```go
package translate

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"time"
)

type Entry struct {
	SkillID        string    `json:"skillId"`
	RelativePath   string    `json:"relativePath"`
	ContentHash    string    `json:"contentHash"`
	SourceLanguage Language  `json:"sourceLanguage"`
	TranslatedText string    `json:"translatedText"`
	Summary        string    `json:"summary,omitempty"`
	UpdatedAt      time.Time `json:"updatedAt"`
}

type Cache struct {
	projectRoot string
	path        string
	Entries     map[string]Entry `json:"entries"`
}

func LoadCache(projectRoot, path string) (*Cache, error) {
	if err := ensureInside(projectRoot, path); err != nil {
		return nil, err
	}
	cache := &Cache{projectRoot: projectRoot, path: path, Entries: map[string]Entry{}}
	data, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return cache, nil
	}
	if err != nil {
		return cache, nil
	}
	if err := json.Unmarshal(data, cache); err != nil {
		return &Cache{projectRoot: projectRoot, path: path, Entries: map[string]Entry{}}, nil
	}
	if cache.Entries == nil {
		cache.Entries = map[string]Entry{}
	}
	cache.projectRoot = projectRoot
	cache.path = path
	return cache, nil
}

func (c *Cache) Get(skillID, relativePath, text string) (Entry, bool) {
	key := Key(skillID, relativePath, HashText(text))
	entry, ok := c.Entries[key]
	return entry, ok
}

func (c *Cache) Set(entry Entry) {
	if c.Entries == nil {
		c.Entries = map[string]Entry{}
	}
	c.Entries[Key(entry.SkillID, entry.RelativePath, entry.ContentHash)] = entry
}

func (c *Cache) Save() error {
	if err := ensureInside(c.projectRoot, c.path); err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(c.path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return err
	}
	tmp := c.path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, c.path)
}

func HashText(text string) string {
	sum := sha256.Sum256([]byte(text))
	return hex.EncodeToString(sum[:])
}

func Key(skillID, relativePath, contentHash string) string {
	return skillID + "|" + filepath.ToSlash(relativePath) + "|" + contentHash
}

func ensureInside(projectRoot, target string) error {
	root, err := filepath.Abs(projectRoot)
	if err != nil {
		return err
	}
	path, err := filepath.Abs(target)
	if err != nil {
		return err
	}
	rel, err := filepath.Rel(root, path)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) || filepath.IsAbs(rel) {
		return errors.New("拒绝读写项目目录外翻译缓存")
	}
	return nil
}
```

- [ ] **Step 5: 给路径配置增加翻译缓存路径**

Modify `internal/config/paths.go`:

```go
type Paths struct {
	WorkDir         string
	StorePath       string
	CachePath       string
	TranslationPath string
	CodexHome       string
}
```

Return:

```go
TranslationPath: filepath.Join(workDir, "data", "translation-cache.json"),
```

Modify `cmd/codex-atlas/main.go` so manager construction becomes:

```go
manager := app.NewManager(paths.StorePath, paths.CachePath, paths.TranslationPath, paths.CodexHome)
```

- [ ] **Step 6: 跑测试**

Run:

```powershell
go test ./internal/translate ./internal/config ./cmd/codex-atlas
```

Expected: PASS after updating constructor in the next task or with temporary compatibility overload. If constructor change breaks app tests, continue to Task 3 immediately and run full tests there.

---

## Task 3: Codex 翻译器与 Manager 集成

**Files:**
- Create: `internal/ai/translator.go`
- Create: `internal/ai/translator_test.go`
- Create: `internal/app/skill_detail.go`
- Create: `internal/app/skill_detail_test.go`
- Modify: `internal/app/manager.go`

- [ ] **Step 1: 写 AI 翻译器测试**

Create `internal/ai/translator_test.go`:

```go
package ai

import (
	"context"
	"path/filepath"
	"testing"
	"time"
)

func TestTranslatorRunsCommandAndParsesJSON(t *testing.T) {
	dir := t.TempDir()
	command := fakeCommand(t, dir, `{"translatedText":"中文正文","summary":"中文摘要"}`, 0)
	translator := Translator{Command: command, WorkDir: dir, Timeout: 2 * time.Second}

	got, err := translator.Translate(context.Background(), TranslateRequest{
		SkillName: "brainstorming",
		Path: "SKILL.md",
		Text: "English body",
	})
	if err != nil {
		t.Fatalf("Translate returned error: %v", err)
	}
	if got.TranslatedText != "中文正文" || got.Summary != "中文摘要" {
		t.Fatalf("response = %+v", got)
	}
	if _, err := os.Stat(filepath.Join(dir, ".tmp", "translation-output.json")); err != nil {
		t.Fatalf("translation output should be written inside project tmp dir: %v", err)
	}
}
```

Use the existing `fakeCommand` helper style from `internal/ai/codex_cli_test.go`; imports must include `os`.

- [ ] **Step 2: 实现 AI 翻译器**

Create `internal/ai/translator.go`:

```go
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
)

type Translator struct {
	Command string
	Args    []string
	WorkDir string
	Timeout time.Duration
}

type TranslateRequest struct {
	SkillName string
	Path      string
	Text      string
}

type TranslateResponse struct {
	TranslatedText string `json:"translatedText"`
	Summary        string `json:"summary"`
}

func (t Translator) Translate(ctx context.Context, request TranslateRequest) (TranslateResponse, error) {
	command := strings.TrimSpace(t.Command)
	if command == "" {
		command = "codex"
	}
	workDir := strings.TrimSpace(t.WorkDir)
	if workDir == "" {
		workDir = "."
	}
	timeout := t.Timeout
	if timeout == 0 {
		timeout = 90 * time.Second
	}
	tmpDir := filepath.Join(workDir, ".tmp")
	if err := os.MkdirAll(tmpDir, 0o755); err != nil {
		return TranslateResponse{}, err
	}
	outputPath := filepath.Join(tmpDir, "translation-output.json")
	args := append([]string{}, t.Args...)
	if command == "codex" && len(args) == 0 {
		args = []string{"exec", "--skip-git-repo-check", "--ephemeral", "-o", outputPath, "-"}
	}

	runCtx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()
	cmd := exec.CommandContext(runCtx, command, args...)
	cmd.Dir = workDir
	cmd.Env = boundedEnv(os.Environ(), tmpDir)
	cmd.Stdin = bytes.NewReader(buildTranslatePrompt(request))
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		if errors.Is(runCtx.Err(), context.DeadlineExceeded) {
			return TranslateResponse{}, fmt.Errorf("翻译超时: %w", runCtx.Err())
		}
		return TranslateResponse{}, fmt.Errorf("翻译命令失败: %w; stderr=%s stdout=%s", err, strings.TrimSpace(stderr.String()), strings.TrimSpace(stdout.String()))
	}
	output := stdout.Bytes()
	if fileOutput, err := os.ReadFile(outputPath); err == nil && len(bytes.TrimSpace(fileOutput)) > 0 {
		output = fileOutput
	} else {
		_ = os.WriteFile(outputPath, stdout.Bytes(), 0o644)
	}
	var response TranslateResponse
	if err := json.Unmarshal(extractJSON(output), &response); err != nil {
		return TranslateResponse{}, fmt.Errorf("翻译输出不是有效 JSON: %w", err)
	}
	if strings.TrimSpace(response.TranslatedText) == "" {
		return TranslateResponse{}, errors.New("翻译输出缺少 translatedText")
	}
	return response, nil
}

func buildTranslatePrompt(request TranslateRequest) []byte {
	prompt := `你是 Codex Atlas 的 skill 文档翻译器。请只输出 JSON，不要输出 Markdown。输出格式必须是 {"translatedText":"...","summary":"..."}。
要求：
1. 将输入内容翻译成自然、准确、适合中国用户阅读的中文。
2. 保留命令、路径、变量名、代码块和 $skill 调用写法。
3. summary 用 60 字以内概括本文档用途。
4. 不要读取或修改任何文件。

Skill: ` + request.SkillName + `
Path: ` + request.Path + `

原文：
` + request.Text
	return []byte(prompt)
}
```

- [ ] **Step 3: 修改 Manager 构造和加载缓存**

Modify `internal/app/manager.go` constructor:

```go
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
	return &Manager{
		ProjectRoot: inferProjectRoot(storePath, cachePath, translationPath),
		StorePath: storePath,
		CachePath: cachePath,
		StatsPath: statsCachePath(cachePath),
		TranslationPath: translationPath,
		CodexHome: codexHome,
		State: domain.NewStore(),
		Stats: map[string]domain.Stats{},
	}
}
```

Add import:

```go
"codex-atlas/internal/translate"
```

In `Load` after stats:

```go
translations, err := translate.LoadCache(m.ProjectRoot, m.TranslationPath)
if err != nil {
	return err
}
m.Translations = translations
```

Update all existing tests and call sites to pass `filepath.Join(dir, "data", "translation-cache.json")`.

- [ ] **Step 4: 写 Manager skill detail 测试**

Create `internal/app/skill_detail_test.go`:

```go
package app

import (
	"context"
	"path/filepath"
	"testing"

	"codex-atlas/internal/domain"
	"codex-atlas/internal/scanner"
)

func TestManagerReadsSkillDocumentAndUsesCachedTranslation(t *testing.T) {
	dir := t.TempDir()
	skillDir := filepath.Join(dir, "skill")
	mustWrite(t, filepath.Join(skillDir, "SKILL.md"), "# Skill\n\nUse this before coding.")

	manager := newTestManager(t, dir)
	manager.Scan = scanner.Result{Abilities: []domain.Ability{{
		ID: "skill:brainstorming", Kind: domain.KindSkill, Name: "brainstorming", SourcePath: filepath.Join(skillDir, "SKILL.md"), Directory: skillDir,
	}}}
	if err := manager.LoadTranslationCacheForTest(); err != nil {
		t.Fatalf("LoadTranslationCacheForTest returned error: %v", err)
	}

	doc, err := manager.ReadSkillFile("skill:brainstorming", "SKILL.md")
	if err != nil {
		t.Fatalf("ReadSkillFile returned error: %v", err)
	}
	if doc.DisplayText != "# Skill\n\nUse this before coding." || doc.HasTranslation {
		t.Fatalf("unexpected doc without cache: %+v", doc)
	}

	manager.SaveTranslationForTest("skill:brainstorming", "SKILL.md", doc.SourceText, "中文正文", "中文摘要")
	doc, err = manager.ReadSkillFile("skill:brainstorming", "SKILL.md")
	if err != nil {
		t.Fatalf("ReadSkillFile after cache returned error: %v", err)
	}
	if doc.DisplayText != "中文正文" || !doc.HasTranslation || doc.Summary != "中文摘要" {
		t.Fatalf("cached doc = %+v", doc)
	}
}
```

In implementation, prefer public helper methods without `ForTest` if practical. The test helper can construct real cache and call exported cache methods.

- [ ] **Step 5: 实现 `internal/app/skill_detail.go`**

Define:

```go
type SkillFileView struct {
	RelativePath string
	Name         string
	Ext          string
	Size         int64
}

type SkillDocumentView struct {
	SkillID        string
	RelativePath   string
	AbsolutePath   string
	SourceText     string
	DisplayText    string
	Summary        string
	Language       translate.Language
	HasTranslation bool
	CanTranslate   bool
}
```

Implement:

```go
func (m *Manager) ListSkillFiles(skillID string) ([]SkillFileView, error)
func (m *Manager) ReadSkillFile(skillID, relativePath string) (SkillDocumentView, error)
func (m *Manager) TranslateSkillFile(ctx context.Context, translator ai.Translator, skillID, relativePath string) (SkillDocumentView, error)
func (m *Manager) TranslatedSummary(ability domain.Ability) string
```

Rules:

- Use `ability.Directory` as root.
- Only allow `ability.Kind == domain.KindSkill`.
- Use `skilldoc.Reader{MaxBytes: 256 * 1024}`.
- `ReadSkillFile` detects language from source text.
- If source language is Chinese, `CanTranslate=false` and `DisplayText=SourceText`.
- If cache hit, `HasTranslation=true` and `DisplayText=TranslatedText`.
- If no cache and English, `CanTranslate=true` and `DisplayText=SourceText`.
- `TranslateSkillFile` re-reads the file, calls translator, saves cache entry, then returns updated view.

- [ ] **Step 6: 主列表摘要中文优先**

In `Abilities`, after `domain.MergeAbility`, apply:

```go
if translated := m.translatedSummaryLocked(merged); translated != "" {
	merged.Summary = translated
}
```

`translatedSummaryLocked` must:

- Only apply to skills.
- Read source `SKILL.md` text only if needed and file is small.
- Return cached `Entry.Summary` first.
- Fall back to existing `merged.Summary`.
- Never call AI automatically.

- [ ] **Step 7: 跑测试**

Run:

```powershell
go test ./internal/ai ./internal/app ./internal/translate ./internal/skilldoc ./cmd/codex-atlas
```

Expected: PASS.

---

## Task 4: 主窗口第二版布局

**Files:**
- Modify: `internal/ui/window_windows.go`
- Modify: `internal/ui/list_model_windows.go`
- Modify: `internal/ui/list_style_windows.go`

- [ ] **Step 1: 调整主窗口尺寸和侧边栏定位**

In `create()`:

```go
Bounds: Rectangle{X: 80, Y: 40, Width: 680, Height: 780},
MinSize: Size{Width: 560, Height: 680},
```

In `placeAsSidebar()`:

```go
width := int32(680)
height := int32(780)
```

Keep screen fitting logic.

- [ ] **Step 2: 重排顶部区域**

Keep existing Walk controls but compress header:

- Remove long explanatory subtitle if it causes vertical pressure.
- Keep brand, shortcut, search, kind switch.
- Keep quick filters.
- Move sort controls into `更多` panel unless already visible.

Acceptance:

- At `680x780` there are at least 5 visible skill rows.
- Bottom detail panel remains visible.
- Search and filters do not overlap.

- [ ] **Step 3: 列表信息改成更清晰的单行/双段文本**

If retaining `ListBox`, update `Value`:

```go
return fmt.Sprintf("%s %s  %s  %s", prefix, name, usageLabel(item.Stats), languageBadge(item))
```

Add:

```go
func languageBadge(item domain.Ability) string {
	if item.Kind != domain.KindSkill {
		return ""
	}
	if strings.Contains(strings.Join(item.Tags, " "), "原生中文") || translate.DetectLanguage(firstNonEmpty(item.Summary, item.Description)) == translate.LanguageChinese {
		return "中文"
	}
	if item.Summary != "" && item.Summary != item.Description {
		return "中文优先"
	}
	return "英文"
}
```

If using `TableView`, create `abilityTableModel` with columns:

- 能力
- 摘要
- 使用
- 语言

Choose `TableView` only if it is stable in Walk smoke test. Otherwise keep `ListBox` to reduce implementation risk.

- [ ] **Step 4: 新增打开详情按钮和双击行为**

Change current double click from `copyTemplate` to detail window:

```go
OnItemActivated: w.openSkillDetail,
```

Add button text:

```go
PushButton{Text: "打开详情", StretchFactor: 1, OnClicked: w.openSkillDetail}
```

Add:

```go
func (w *atlasWindow) openSkillDetail() {
	item, ok := w.selected()
	if !ok || item.Kind != domain.KindSkill {
		return
	}
	if err := showSkillDetailWindow(w.MainWindow, w.manager, w.workDir, item); err != nil {
		w.showError("打开 Skill 详情失败", err)
	}
}
```

- [ ] **Step 5: 手动 GUI smoke**

Run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build.ps1
.\dist\codex-atlas.exe
```

Verify manually:

- Window opens wider and taller.
- List shows usage and language state.
- Double click a skill opens detail window.
- Copy template remains available from button.
- CPU and memory stay roughly in first版量级。

---

## Task 5: Skill 详情弹窗

**Files:**
- Create: `internal/ui/skill_detail_windows.go`
- Modify: `internal/ui/window_windows.go`

- [ ] **Step 1: 创建详情窗口结构**

Create a `skillDetailWindow` struct:

```go
type skillDetailWindow struct {
	*walk.Dialog
	manager *app.Manager
	workDir string
	item domain.Ability

	fileList *walk.ListBox
	fileModel *skillFileListModel
	contentEdit *walk.TextEdit
	sourceButton *walk.PushButton
	translationButton *walk.PushButton
	translateButton *walk.PushButton
	copyButton *walk.PushButton
	statusLabel *walk.Label

	current app.SkillDocumentView
	showTranslation bool
	busy bool
}
```

- [ ] **Step 2: 实现文件列表模型**

```go
type skillFileListModel struct {
	walk.ListModelBase
	items []app.SkillFileView
}

func (m *skillFileListModel) ItemCount() int { return len(m.items) }
func (m *skillFileListModel) Value(index int) interface{} {
	if index < 0 || index >= len(m.items) {
		return ""
	}
	item := m.items[index]
	return item.RelativePath
}
func (m *skillFileListModel) setItems(items []app.SkillFileView) {
	m.items = items
	m.PublishItemsReset()
}
```

- [ ] **Step 3: 实现弹窗布局**

Use `Dialog` with:

- Title: `Skill 详情 · <name>`
- Bounds: `Rectangle{Width: 990, Height: 720}`
- MinSize: `Size{Width: 760, Height: 560}`
- Main layout: vertical header plus horizontal split.
- Left: file list and path info.
- Right: toolbar and read-only `TextEdit`.

Toolbar buttons:

- `原文`
- `译文`
- `翻译` or `重新翻译`
- `复制`
- `打开目录`

- [ ] **Step 4: 默认显示规则**

On file selection:

```go
doc, err := w.manager.ReadSkillFile(w.item.ID, selected.RelativePath)
w.current = doc
w.showTranslation = doc.HasTranslation
w.renderDocument()
```

`renderDocument`:

- If `showTranslation && current.HasTranslation`, show translated text.
- Else show source text.
- Disable/hide `translateButton` when `!current.CanTranslate && !current.HasTranslation`.
- Use status text:
  - `英文，有译文缓存`
  - `英文，未翻译`
  - `中文，无需翻译`
  - `文件无法预览`

- [ ] **Step 5: 翻译按钮异步执行**

```go
func (w *skillDetailWindow) translateCurrent() {
	if w.busy || w.current.RelativePath == "" {
		return
	}
	w.setBusy(true)
	go func() {
		translator := ai.Translator{WorkDir: w.workDir, Timeout: 2 * time.Minute}
		doc, err := w.manager.TranslateSkillFile(context.Background(), translator, w.item.ID, w.current.RelativePath)
		w.Synchronize(func() {
			w.setBusy(false)
			if err != nil {
				walk.MsgBox(w, "翻译失败", err.Error(), walk.MsgBoxIconError)
				return
			}
			w.current = doc
			w.showTranslation = true
			w.renderDocument()
		})
	}()
}
```

The implementation must disable file list and buttons while translating.

- [ ] **Step 6: 详情窗口 smoke**

Manual checks:

- Open `brainstorming`.
- `SKILL.md` selected by default.
- Cached translation displays by default if cache exists.
- Clicking `原文` shows original English.
- Clicking `译文` returns to Chinese.
- Chinese file hides or disables translate action.
- Binary/large file shows a clear preview error and does not crash.

---

## Task 6: 构建脚本、文档和缓存打包

**Files:**
- Modify: `scripts/build.ps1`
- Modify: `README.md`
- Modify: `docs/使用说明.md`
- Modify: `docs/方案设计.md`
- Modify: `docs/实施计划.md`
- Modify: `docs/开发记录.md`

- [ ] **Step 1: 构建脚本复制翻译缓存**

In `scripts/build.ps1`, extend data copy list:

```powershell
foreach ($name in @("last-scan.json", "stats-cache.json", "translation-cache.json")) {
    $source = Join-Path $dataDir $name
    if (Test-Path -LiteralPath $source) {
        Copy-Item -LiteralPath $source -Destination $distData -Force
    }
}
```

- [ ] **Step 2: 文档更新**

README must mention:

- 主窗口 v2 默认更宽。
- Skill 详情弹窗。
- `data/translation-cache.json`。
- 翻译通过本机 `codex exec --ephemeral` 手动触发。

`docs/使用说明.md` must include:

- 如何打开详情。
- 原文/译文切换。
- 哪些情况下显示翻译按钮。
- 缓存命中默认中文。

`docs/方案设计.md` must include:

- `internal/skilldoc`
- `internal/translate`
- `internal/ai.Translator`
- 翻译缓存 key 规则。

`docs/开发记录.md` add `2026-07-05 第二版迭代计划与实现` section.

- [ ] **Step 3: 全量验证**

Run:

```powershell
go test ./...
powershell -ExecutionPolicy Bypass -File .\scripts\build.ps1
.\.tmp\codex-atlas-console.exe --smoke-scan
```

Expected:

- Tests pass.
- Build produces `dist/codex-atlas.exe`.
- Smoke scan exits 0 and prints abilities/skills.

- [ ] **Step 4: GUI smoke 截图**

Run `dist\codex-atlas.exe` and capture screenshots:

- Main window v2.
- Skill detail window with cached translation.
- Skill detail window with English no cache.
- Skill detail window with Chinese file no translate button.

Save screenshots to `.tmp/` only.

- [ ] **Step 5: Dalton 子 agent 复审**

Ask Dalton to review:

- 是否满足第二版需求。
- 是否有路径越界或写入 `.codex` 的风险。
- 翻译缓存是否会误显示过期译文。
- UI 是否保持轻量，没有引入重依赖。
- 测试是否覆盖关键分支。

Fix Critical and Important findings before final delivery.

- [ ] **Step 6: 提交和推送**

After all verification passes:

```powershell
git -c core.quotepath=false status -sb
git diff --stat
git add internal cmd scripts docs README.md data/translation-cache.json
git commit -m "实现 Codex Atlas 第二版 Skill 阅读与翻译" -m "- 扩大并重排主窗口，新增独立 Skill 详情弹窗" -m "- 增加 skill 文件夹只读浏览、文件预览、语言判断和翻译缓存" -m "- 接入本机 codex 一次性翻译，并更新构建脚本和中文文档"
git push
```

Only include `data/translation-cache.json` if it contains useful personal cache and the user still wants executable/self用仓库直接可用。Do not commit `.tmp/` or `.cache/`.

---

## Final Acceptance Criteria

- 主窗口默认不小于 `680x780`，信息显示不拥挤。
- 双击或点击按钮可打开 Skill 详情弹窗。
- 弹窗左侧显示 skill 文件夹文件，右侧显示文件内容。
- 已缓存英文文件默认显示中文译文。
- 英文无缓存时默认原文并显示翻译按钮。
- 中文文件不显示翻译按钮。
- 翻译调用只在用户点击时发生，不自动批量翻译。
- 翻译缓存写入当前项目 `data/translation-cache.json`。
- `.codex` 目录只读，没有任何写入、删除或移动。
- `go test ./...` 通过。
- `scripts/build.ps1` 通过。
- GUI smoke 通过，CPU/内存仍保持轻量。
- Dalton 复审无 Critical/Important 未修复问题。

## Self-Review

- Spec coverage: 第二版的三个核心需求都有任务覆盖。主界面在 Task 4，独立详情弹窗和文件浏览在 Task 1/5，翻译和缓存以及中文优先在 Task 2/3。
- Placeholder scan: 本计划没有遗留占位表达。每个实现任务都给出文件、接口、测试和命令。
- Type consistency: `skilldoc.Reader`、`translate.Cache`、`ai.Translator`、`app.SkillDocumentView` 在后续任务中名称一致。
- Scope check: 功能属于同一条用户阅读流程，可以作为一个版本计划执行，不需要拆成多个独立项目。
