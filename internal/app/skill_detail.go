package app

import (
	"context"
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"codex-atlas/internal/ai"
	"codex-atlas/internal/domain"
	"codex-atlas/internal/skilldoc"
	"codex-atlas/internal/translate"
)

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

func (m *Manager) ListSkillFiles(skillID string) ([]SkillFileView, error) {
	ability, err := m.skillAbilitySnapshot(skillID)
	if err != nil {
		return nil, err
	}
	files, err := skilldoc.Reader{}.List(ability.Directory)
	if err != nil {
		return nil, err
	}
	out := make([]SkillFileView, 0, len(files))
	for _, file := range files {
		out = append(out, SkillFileView{
			RelativePath: file.RelativePath,
			Name:         file.Name,
			Ext:          file.Ext,
			Size:         file.Size,
		})
	}
	return out, nil
}

func (m *Manager) ReadSkillFile(skillID, relativePath string) (SkillDocumentView, error) {
	ability, err := m.skillAbilitySnapshot(skillID)
	if err != nil {
		return SkillDocumentView{}, err
	}
	doc, err := skilldoc.Reader{}.Read(ability.Directory, relativePath)
	if err != nil {
		return SkillDocumentView{}, err
	}

	var entry translate.Entry
	var hit bool
	m.mu.Lock()
	if m.Translations != nil {
		entry, hit = m.Translations.Get(skillID, doc.RelativePath, doc.Text)
	}
	m.mu.Unlock()
	if hit {
		// 缓存命中时默认展示中文译文，但不会在读取文档时自动调用 AI。
		return documentViewFromEntry(skillID, doc, entry), nil
	}
	return documentViewFromEntry(skillID, doc, translate.Entry{}), nil
}

func (m *Manager) TranslateSkillFile(ctx context.Context, translator ai.Translator, skillID, relativePath string) (SkillDocumentView, error) {
	ability, err := m.skillAbilitySnapshot(skillID)
	if err != nil {
		return SkillDocumentView{}, err
	}
	doc, err := skilldoc.Reader{}.Read(ability.Directory, relativePath)
	if err != nil {
		return SkillDocumentView{}, err
	}
	language := translate.DetectLanguage(doc.Text)
	if language == translate.LanguageChinese {
		return SkillDocumentView{}, errors.New("中文源文件不需要翻译")
	}

	// 外部命令边界：翻译命令可能很慢，运行期间绝不持有 Manager 锁。
	response, err := translator.Translate(ctx, ai.TranslateRequest{
		SkillName: ability.Name,
		Path:      doc.RelativePath,
		Text:      doc.Text,
	})
	if err != nil {
		return SkillDocumentView{}, err
	}

	entry := translate.Entry{
		SkillID:        skillID,
		RelativePath:   doc.RelativePath,
		ContentHash:    translate.HashText(doc.Text),
		SourceLanguage: language,
		TranslatedText: response.TranslatedText,
		Summary:        response.Summary,
		UpdatedAt:      time.Now(),
	}

	m.mu.Lock()
	defer m.mu.Unlock()
	if err := m.ensureTranslationsLocked(); err != nil {
		return SkillDocumentView{}, err
	}
	// 锁范围只覆盖内存缓存更新和落盘，外部 AI 调用已经在锁外完成。
	if err := m.Translations.Set(entry); err != nil {
		return SkillDocumentView{}, err
	}
	if err := m.Translations.Save(); err != nil {
		return SkillDocumentView{}, err
	}
	return documentViewFromEntry(skillID, doc, entry), nil
}

func (m *Manager) TranslatedSummary(ability domain.Ability) string {
	if ability.Kind != domain.KindSkill || strings.TrimSpace(ability.Directory) == "" {
		return ""
	}
	doc, err := skilldoc.Reader{}.Read(ability.Directory, "SKILL.md")
	if err != nil {
		return ""
	}

	m.mu.Lock()
	defer m.mu.Unlock()
	if m.Translations == nil {
		return ""
	}
	entry, ok := m.Translations.Get(ability.ID, doc.RelativePath, doc.Text)
	if !ok || strings.TrimSpace(entry.Summary) == "" {
		return ""
	}
	// 缓存命中：这里只读取既有翻译摘要，不会自动调用 AI。
	return strings.TrimSpace(entry.Summary)
}

func (m *Manager) skillAbilitySnapshot(skillID string) (domain.Ability, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	for _, ability := range m.Scan.Abilities {
		if ability.ID != skillID {
			continue
		}
		if ability.Kind != domain.KindSkill {
			return domain.Ability{}, fmt.Errorf("能力 %s 不是 skill，不能读取 skill 文件", skillID)
		}
		if strings.TrimSpace(ability.Directory) == "" {
			return domain.Ability{}, fmt.Errorf("skill %s 缺少目录，不能读取文件", skillID)
		}
		return ability, nil
	}
	return domain.Ability{}, fmt.Errorf("找不到 skill: %s", skillID)
}

func documentViewFromEntry(skillID string, doc skilldoc.Document, entry translate.Entry) SkillDocumentView {
	language := translate.DetectLanguage(doc.Text)
	view := SkillDocumentView{
		SkillID:      skillID,
		RelativePath: doc.RelativePath,
		AbsolutePath: doc.AbsolutePath,
		SourceText:   doc.Text,
		DisplayText:  doc.Text,
		Language:     language,
		CanTranslate: language != translate.LanguageChinese,
	}
	if strings.TrimSpace(entry.TranslatedText) != "" {
		view.DisplayText = strings.TrimSpace(entry.TranslatedText)
		view.Summary = strings.TrimSpace(entry.Summary)
		view.HasTranslation = true
	}
	return view
}

func translationCachePath(cachePath string) string {
	if strings.TrimSpace(cachePath) == "" {
		return ""
	}
	return filepath.Join(filepath.Dir(cachePath), "translation-cache.json")
}

var errTranslationsUnavailable = errors.New("翻译缓存未初始化")
