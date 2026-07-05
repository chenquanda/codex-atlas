package store

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"codex-atlas/internal/domain"
)

type JSONStore struct {
	path        string
	projectRoot string
}

func NewJSONStore(path string) JSONStore {
	return JSONStore{path: path}
}

func NewBoundedJSONStore(projectRoot, path string) (JSONStore, error) {
	if err := ensureInside(projectRoot, path); err != nil {
		return JSONStore{}, err
	}
	return JSONStore{path: path, projectRoot: projectRoot}, nil
}

// Load 读取项目内存储文件；文件不存在时返回初始化后的空状态。
func (s JSONStore) Load() (domain.Store, error) {
	if err := ensureInside(s.projectRoot, s.path); err != nil {
		return domain.Store{}, err
	}
	data, err := os.ReadFile(s.path)
	if errors.Is(err, os.ErrNotExist) {
		return domain.NewStore(), nil
	}
	if err != nil {
		return domain.Store{}, err
	}
	var state domain.Store
	if err := json.Unmarshal(data, &state); err != nil {
		return domain.Store{}, err
	}
	return normalize(state), nil
}

// Save 使用同目录临时文件再 rename，降低写入中断造成 JSON 损坏的概率。
func (s JSONStore) Save(state domain.Store) error {
	if err := ensureInside(s.projectRoot, s.path); err != nil {
		return err
	}
	state = normalize(state)
	if err := os.MkdirAll(filepath.Dir(s.path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return err
	}
	tmp := s.path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, s.path)
}

// ensureInside 用于生产路径边界：设置 projectRoot 后，任何读写路径都必须落在项目目录内。
func ensureInside(projectRoot, target string) error {
	if strings.TrimSpace(projectRoot) == "" {
		return nil
	}
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
		return fmt.Errorf("拒绝写入项目目录外路径: %s", target)
	}
	return nil
}

func normalize(state domain.Store) domain.Store {
	if state.Version == 0 {
		state.Version = 1
	}
	if state.Users == nil {
		state.Users = map[string]domain.UserData{}
	}
	if state.AI == nil {
		state.AI = map[string]domain.AIData{}
	}
	if state.Settings == nil {
		state.Settings = map[string]string{}
	}
	return state
}
