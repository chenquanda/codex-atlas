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

var ErrCachePathOutside = errors.New("翻译缓存路径不在项目目录内")

type Cache struct {
	Version int              `json:"version"`
	Entries map[string]Entry `json:"entries"`

	projectRoot string
	path        string
}

type Entry struct {
	SkillID        string    `json:"skillId"`
	RelativePath   string    `json:"relativePath"`
	ContentHash    string    `json:"contentHash"`
	SourceLanguage Language  `json:"sourceLanguage"`
	TranslatedText string    `json:"translatedText"`
	Summary        string    `json:"summary,omitempty"`
	UpdatedAt      time.Time `json:"updatedAt"`
}

func HashText(text string) string {
	sum := sha256.Sum256([]byte(text))
	return hex.EncodeToString(sum[:])
}

func LoadCache(projectRoot, path string) (*Cache, error) {
	rootAbs, pathAbs, err := resolveCachePath(projectRoot, path)
	if err != nil {
		return nil, err
	}

	cache := newCache(rootAbs, pathAbs)
	data, err := readCacheData(rootAbs, pathAbs)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return cache, nil
		}
		return nil, err
	}
	if err := json.Unmarshal(data, cache); err != nil {
		// 损坏 JSON 不能阻断启动，返回空缓存让调用方按 miss 重新翻译。
		return newCache(rootAbs, pathAbs), nil
	}
	cache.projectRoot = rootAbs
	cache.path = pathAbs
	cache.ensureEntries()
	return cache, nil
}

func (c *Cache) Get(skillID, relativePath, text string) (Entry, bool) {
	return c.GetByHash(skillID, relativePath, HashText(text))
}

func (c *Cache) GetByHash(skillID, relativePath, contentHash string) (Entry, bool) {
	if c == nil {
		return Entry{}, false
	}
	normalizedPath, err := normalizeSafeRelativePath(relativePath)
	if err != nil {
		return Entry{}, false
	}
	c.ensureEntries()
	entry, ok := c.Entries[cacheKey(skillID, normalizedPath, contentHash)]
	if !ok {
		return Entry{}, false
	}
	return entry, true
}

func (c *Cache) Set(entry Entry) error {
	c.ensureEntries()
	normalizedPath, err := normalizeSafeRelativePath(entry.RelativePath)
	if err != nil {
		return err
	}
	entry.RelativePath = normalizedPath
	c.Entries[cacheKey(entry.SkillID, entry.RelativePath, entry.ContentHash)] = entry
	return nil
}

func (c *Cache) Save() error {
	if c == nil {
		return errors.New("翻译缓存为空")
	}
	_, pathAbs, err := resolveCachePath(c.projectRoot, c.path)
	if err != nil {
		return err
	}
	c.ensureEntries()

	data, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return err
	}

	dir := filepath.Dir(pathAbs)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	tempFile, err := os.CreateTemp(dir, ".translation-cache-*.tmp")
	if err != nil {
		return err
	}
	tempPath := tempFile.Name()
	cleanupTemp := true
	defer func() {
		if cleanupTemp {
			_ = os.Remove(tempPath)
		}
	}()

	if _, err := tempFile.Write(data); err != nil {
		_ = tempFile.Close()
		return err
	}
	if err := tempFile.Close(); err != nil {
		return err
	}
	// 先写同目录临时文件，再用备份保护替换；Windows 不能把临时文件直接 rename 到已有文件。
	// remove+rename 的窗口里如果失败，必须从 backup 恢复旧缓存，避免丢失可用 JSON。
	if err := replaceWithBackup(tempPath, pathAbs); err != nil {
		return err
	}
	cleanupTemp = false
	return nil
}

func newCache(projectRoot, path string) *Cache {
	return &Cache{
		Version:     1,
		Entries:     map[string]Entry{},
		projectRoot: projectRoot,
		path:        path,
	}
}

func (c *Cache) ensureEntries() {
	if c.Version == 0 {
		c.Version = 1
	}
	if c.Entries == nil {
		c.Entries = map[string]Entry{}
	}
}

func cacheKey(skillID, relativePath, contentHash string) string {
	// relativePath 统一为 slash，确保 Windows 反斜杠输入和扫描得到的相对路径命中同一条缓存。
	return skillID + "|" + normalizeRelativePath(relativePath) + "|" + contentHash
}

func normalizeRelativePath(path string) string {
	return strings.Trim(strings.ReplaceAll(path, `\`, "/"), "/")
}

func normalizeSafeRelativePath(path string) (string, error) {
	if strings.TrimSpace(path) == "" {
		return "", ErrCachePathOutside
	}
	// 先拒绝根路径和绝对路径，避免后续 slash 归一把危险路径“洗白”为相对路径写入 JSON。
	if strings.HasPrefix(path, "/") || strings.HasPrefix(path, `\`) {
		return "", ErrCachePathOutside
	}
	nativePath := filepath.FromSlash(strings.ReplaceAll(path, `\`, "/"))
	if filepath.IsAbs(nativePath) {
		return "", ErrCachePathOutside
	}
	slashPath := strings.ReplaceAll(path, `\`, "/")
	for _, part := range strings.Split(slashPath, "/") {
		if part == ".." {
			return "", ErrCachePathOutside
		}
	}
	normalizedPath := normalizeRelativePath(path)
	if normalizedPath == "" {
		return "", ErrCachePathOutside
	}
	return normalizedPath, nil
}

func resolveCachePath(projectRoot, path string) (string, string, error) {
	rootAbs, err := filepath.Abs(projectRoot)
	if err != nil {
		return "", "", err
	}
	pathForAbs := path
	if !filepath.IsAbs(pathForAbs) {
		cwdRelativeAbs, err := filepath.Abs(pathForAbs)
		if err != nil {
			return "", "", err
		}
		if isInsideRoot(rootAbs, cwdRelativeAbs) {
			pathForAbs = cwdRelativeAbs
		} else {
			pathForAbs = filepath.Join(rootAbs, pathForAbs)
		}
	}
	pathAbs, err := filepath.Abs(pathForAbs)
	if err != nil {
		return "", "", err
	}
	// 路径边界只信任 filepath.Rel 的结果，避免 .. 和同名前缀目录绕过项目根目录限制。
	if !isInsideRoot(rootAbs, pathAbs) {
		return "", "", ErrCachePathOutside
	}
	if err := ensureExistingPathInsideRoot(rootAbs, pathAbs); err != nil {
		return "", "", err
	}
	return rootAbs, pathAbs, nil
}

func readCacheData(rootAbs, pathAbs string) ([]byte, error) {
	data, err := os.ReadFile(pathAbs)
	if err == nil {
		return data, nil
	}
	if !errors.Is(err, os.ErrNotExist) {
		return nil, err
	}

	backupPath := cacheBackupPath(pathAbs)
	if _, _, resolveErr := resolveCachePath(rootAbs, backupPath); resolveErr != nil {
		return nil, resolveErr
	}
	backupData, backupErr := os.ReadFile(backupPath)
	if backupErr != nil {
		return nil, backupErr
	}
	return backupData, nil
}

func replaceWithBackup(tempPath, pathAbs string) error {
	backupPath := cacheBackupPath(pathAbs)
	hadExisting := false
	if _, err := os.Stat(pathAbs); err == nil {
		hadExisting = true
		if err := copyFileAtomic(pathAbs, backupPath); err != nil {
			return err
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}

	if hadExisting {
		if err := os.Remove(pathAbs); err != nil {
			return err
		}
	}
	if err := os.Rename(tempPath, pathAbs); err != nil {
		if hadExisting {
			restoreErr := copyFileAtomic(backupPath, pathAbs)
			if restoreErr != nil {
				return errors.Join(err, restoreErr)
			}
		}
		return err
	}
	return nil
}

func cacheBackupPath(pathAbs string) string {
	return pathAbs + ".bak"
}

func copyFileAtomic(srcPath, dstPath string) error {
	src, err := os.Open(srcPath)
	if err != nil {
		return err
	}
	defer src.Close()

	dir := filepath.Dir(dstPath)
	tempFile, err := os.CreateTemp(dir, ".translation-cache-backup-*.tmp")
	if err != nil {
		return err
	}
	tempPath := tempFile.Name()
	cleanupTemp := true
	defer func() {
		if cleanupTemp {
			_ = os.Remove(tempPath)
		}
	}()

	if _, err := tempFile.ReadFrom(src); err != nil {
		_ = tempFile.Close()
		return err
	}
	if err := tempFile.Close(); err != nil {
		return err
	}
	if err := os.Remove(dstPath); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if err := os.Rename(tempPath, dstPath); err != nil {
		return err
	}
	cleanupTemp = false
	return nil
}

func ensureExistingPathInsideRoot(rootAbs, pathAbs string) error {
	rootEval, err := filepath.EvalSymlinks(rootAbs)
	if err != nil {
		return err
	}
	existingPath, err := nearestExistingPath(pathAbs)
	if err != nil {
		return err
	}
	existingEval, err := filepath.EvalSymlinks(existingPath)
	if err != nil {
		return err
	}
	// 词法边界只能拦截 ..；这里再校验已有路径组件的真实落点，拒绝 data symlink/junction 指向项目外。
	if !isInsideRoot(rootEval, existingEval) {
		return ErrCachePathOutside
	}
	return nil
}

func nearestExistingPath(pathAbs string) (string, error) {
	current := pathAbs
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

func isInsideRoot(rootAbs, pathAbs string) bool {
	relativePath, err := filepath.Rel(rootAbs, pathAbs)
	if err != nil {
		return false
	}
	if relativePath == "." {
		return true
	}
	return relativePath != ".." && !strings.HasPrefix(relativePath, ".."+string(os.PathSeparator))
}
