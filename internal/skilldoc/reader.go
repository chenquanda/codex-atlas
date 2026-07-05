package skilldoc

import (
	"bytes"
	"errors"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

const (
	defaultMaxBytes int64 = 256 * 1024
	defaultMaxFiles       = 300
	defaultMaxDepth       = 6
)

var (
	ErrPathOutside  = errors.New("文件路径不在 skill 目录内")
	ErrFileTooLarge = errors.New("文件过大，无法预览")
	ErrBinaryFile   = errors.New("二进制文件无法预览")
)

type Reader struct {
	MaxBytes int64
	MaxFiles int
	MaxDepth int
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
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return nil, err
	}
	var files []FileInfo
	maxBytes := r.maxBytes()
	maxFiles := r.maxFiles()
	maxDepth := r.maxDepth()
	err = filepath.WalkDir(rootAbs, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		relativePath, err := filepath.Rel(rootAbs, path)
		if err != nil {
			return err
		}
		relativePath = filepath.ToSlash(relativePath)
		if entry.IsDir() {
			if relativePath == "." {
				return nil
			}
			if shouldSkipListDir(entry.Name()) {
				return filepath.SkipDir
			}
			// 深度限制保护 UI：过深的源码/依赖树通常不是 skill 文档，直接跳过其子节点。
			if relativeDepth(relativePath) >= maxDepth {
				return filepath.SkipDir
			}
			return nil
		}
		if len(files) >= maxFiles {
			return filepath.SkipAll
		}
		if entry.Type()&os.ModeSymlink != 0 {
			return nil
		}
		if !isPreviewFile(relativePath) {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return nil
		}
		if info.Size() > maxBytes {
			return nil
		}
		files = append(files, FileInfo{
			RelativePath: relativePath,
			Name:         filepath.Base(path),
			Ext:          normalizedExt(path),
			Size:         info.Size(),
		})
		if len(files) >= maxFiles {
			return filepath.SkipAll
		}
		return nil
	})
	if err != nil {
		return nil, err
	}

	sort.Slice(files, func(i, j int) bool { return fileInfoLess(files, i, j) })

	return files, nil
}

func (r Reader) Read(root, relativePath string) (Document, error) {
	rootAbs, targetAbs, err := resolveInsideRoot(root, relativePath)
	if err != nil {
		return Document{}, err
	}
	if err := rejectSymlinkComponents(rootAbs, relativePath); err != nil {
		return Document{}, err
	}

	file, err := os.Open(targetAbs)
	if err != nil {
		return Document{}, err
	}
	defer file.Close()

	// Windows 标准库没有跨平台的 no-follow Open；打开后再次检查路径组件和文件句柄状态，
	// 尽量收敛 Lstat 到 Open 间的本地竞态。攻击者若能在两次检查间反复替换路径，仍存在
	// 极窄竞态；本模块只读预览用户本机 skill 文件，不写入也不执行内容，因此该残余风险可接受。
	if err := rejectSymlinkComponents(rootAbs, relativePath); err != nil {
		return Document{}, err
	}
	info, err := file.Stat()
	if err != nil {
		return Document{}, err
	}
	if !info.Mode().IsRegular() {
		// Read 只预览普通文本文件，目录、设备文件等非普通文件不进入读取流程。
		return Document{}, os.ErrInvalid
	}

	maxBytes := r.maxBytes()
	// 大文件先通过文件句柄元数据拒绝，避免为了预览一次性读入过多内容。
	if info.Size() > maxBytes {
		return Document{}, ErrFileTooLarge
	}

	// 即使文件在 Stat 后被替换，也最多读取 MaxBytes+1 字节来确认边界。
	data, err := io.ReadAll(io.LimitReader(file, maxBytes+1))
	if err != nil {
		return Document{}, err
	}
	if int64(len(data)) > maxBytes {
		return Document{}, ErrFileTooLarge
	}
	// NUL 字节通常意味着二进制内容，文本预览明确拒绝这类文件。
	if bytes.Contains(data, []byte{0}) {
		return Document{}, ErrBinaryFile
	}

	normalizedRelativePath, err := filepath.Rel(rootAbs, targetAbs)
	if err != nil {
		return Document{}, err
	}
	return Document{
		RelativePath: filepath.ToSlash(normalizedRelativePath),
		AbsolutePath: targetAbs,
		Text:         string(data),
		Size:         int64(len(data)),
	}, nil
}

func (r Reader) maxBytes() int64 {
	if r.MaxBytes > 0 {
		return r.MaxBytes
	}
	return defaultMaxBytes
}

func (r Reader) maxFiles() int {
	if r.MaxFiles > 0 {
		return r.MaxFiles
	}
	return defaultMaxFiles
}

func (r Reader) maxDepth() int {
	if r.MaxDepth > 0 {
		return r.MaxDepth
	}
	return defaultMaxDepth
}

func resolveInsideRoot(root, relativePath string) (string, string, error) {
	if err := rejectUnsafeRelativePath(relativePath); err != nil {
		return "", "", err
	}

	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return "", "", err
	}

	nativeRelativePath := filepath.FromSlash(relativePath)
	// 只接受相对路径；绝对路径会绕过 root 拼接，必须直接拒绝。
	if filepath.IsAbs(nativeRelativePath) {
		return "", "", ErrPathOutside
	}

	targetAbs, err := filepath.Abs(filepath.Join(rootAbs, nativeRelativePath))
	if err != nil {
		return "", "", err
	}
	// 路径边界使用 filepath.Rel 判断，避免 ..、同名前缀目录等穿越 root。
	if !isInsideRoot(rootAbs, targetAbs) {
		return "", "", ErrPathOutside
	}

	return rootAbs, targetAbs, nil
}

func rejectUnsafeRelativePath(relativePath string) error {
	if relativePath == "" {
		return ErrPathOutside
	}
	// 词法层先拒绝绝对路径、盘符路径、根路径写法，避免后续 Join/Abs 将其解释到 root 外。
	if strings.HasPrefix(relativePath, "/") || strings.HasPrefix(relativePath, `\`) {
		return ErrPathOutside
	}

	nativeRelativePath := filepath.FromSlash(relativePath)
	if filepath.IsAbs(nativeRelativePath) {
		return ErrPathOutside
	}

	slashPath := strings.ReplaceAll(relativePath, `\`, "/")
	for _, part := range strings.Split(slashPath, "/") {
		if part == ".." {
			return ErrPathOutside
		}
	}

	return nil
}

func rejectSymlinkComponents(rootAbs, relativePath string) error {
	current := rootAbs
	for _, part := range relativePathParts(relativePath) {
		current = filepath.Join(current, part)
		info, err := os.Lstat(current)
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			// 任一路径组件是符号链接都可能把后续 os.Open 引到 root 外部，读取前必须拒绝。
			return ErrPathOutside
		}
	}
	return nil
}

func relativePathParts(relativePath string) []string {
	slashPath := strings.ReplaceAll(relativePath, `\`, "/")
	parts := strings.Split(slashPath, "/")
	out := parts[:0]
	for _, part := range parts {
		if part == "" || part == "." {
			continue
		}
		out = append(out, part)
	}
	return out
}

func isInsideRoot(rootAbs, targetAbs string) bool {
	relativePath, err := filepath.Rel(rootAbs, targetAbs)
	if err != nil {
		return false
	}
	if relativePath == "." {
		return true
	}
	return relativePath != ".." && !strings.HasPrefix(relativePath, ".."+string(os.PathSeparator))
}

func normalizedExt(path string) string {
	return strings.TrimPrefix(strings.ToLower(filepath.Ext(path)), ".")
}

func shouldSkipListDir(name string) bool {
	switch strings.ToLower(name) {
	case ".git", ".hg", ".svn", ".cache", ".tmp", ".next", ".turbo",
		"node_modules", "vendor", "dist", "build", "out", "coverage",
		"target", "bin", "obj", "tmp":
		return true
	default:
		return false
	}
}

func isPreviewFile(relativePath string) bool {
	ext := normalizedExt(relativePath)
	if ext != "" {
		return previewExtensions[ext]
	}
	switch strings.ToLower(filepath.Base(relativePath)) {
	case "license", "notice", "dockerfile", "makefile", "justfile", "gemfile":
		return true
	default:
		return false
	}
}

var previewExtensions = map[string]bool{
	"bat":      true,
	"c":        true,
	"cmd":      true,
	"conf":     true,
	"cpp":      true,
	"cs":       true,
	"css":      true,
	"csv":      true,
	"go":       true,
	"h":        true,
	"html":     true,
	"ini":      true,
	"java":     true,
	"js":       true,
	"json":     true,
	"jsx":      true,
	"kt":       true,
	"lua":      true,
	"md":       true,
	"markdown": true,
	"mjs":      true,
	"ps1":      true,
	"py":       true,
	"rb":       true,
	"rs":       true,
	"scss":     true,
	"sh":       true,
	"sql":      true,
	"svelte":   true,
	"swift":    true,
	"toml":     true,
	"ts":       true,
	"tsx":      true,
	"txt":      true,
	"vue":      true,
	"xml":      true,
	"yaml":     true,
	"yml":      true,
}

func relativeDepth(relativePath string) int {
	count := 0
	for _, part := range strings.Split(strings.ReplaceAll(relativePath, `\`, "/"), "/") {
		if part == "" || part == "." {
			continue
		}
		count++
	}
	return count
}

func fileInfoLess(files []FileInfo, i, j int) bool {
	left := files[i].RelativePath
	right := files[j].RelativePath
	if left == right {
		return false
	}
	if left == "SKILL.md" {
		return true
	}
	if right == "SKILL.md" {
		return false
	}
	return left < right
}
