//go:build windows

package ui

import (
	"context"
	"fmt"
	"strings"
	"sync/atomic"
	"time"

	"codex-atlas/internal/ai"
	"codex-atlas/internal/app"
	"codex-atlas/internal/domain"
	"codex-atlas/internal/translate"

	"github.com/lxn/walk"
	. "github.com/lxn/walk/declarative"
)

type skillFileListModel struct {
	walk.ListModelBase
	items []app.SkillFileView
}

func (m *skillFileListModel) ItemCount() int {
	return len(m.items)
}

func (m *skillFileListModel) Value(index int) interface{} {
	if index < 0 || index >= len(m.items) {
		return ""
	}
	return m.items[index].RelativePath
}

func (m *skillFileListModel) setItems(items []app.SkillFileView) {
	m.items = items
	m.PublishItemsReset()
}

func (m *skillFileListModel) current(index int) (app.SkillFileView, bool) {
	if index < 0 || index >= len(m.items) {
		return app.SkillFileView{}, false
	}
	return m.items[index], true
}

func (m *skillFileListModel) defaultIndex() int {
	for i, item := range m.items {
		if strings.EqualFold(item.RelativePath, "SKILL.md") {
			return i
		}
	}
	if len(m.items) > 0 {
		return 0
	}
	return -1
}

type skillDetailWindow struct {
	*walk.Dialog

	manager *app.Manager
	workDir string
	item    domain.Ability

	fileList  *walk.ListBox
	fileModel *skillFileListModel
	pathLabel *walk.Label
	status    *walk.Label
	content   *walk.TextEdit

	sourceButton      *walk.PushButton
	translationButton *walk.PushButton
	translateButton   *walk.PushButton
	copyButton        *walk.PushButton
	openDirButton     *walk.PushButton
	closeButton       *walk.PushButton

	current            app.SkillDocumentView
	showingTranslation bool
	busy               bool
	closed             atomic.Bool
	translateCancel    context.CancelFunc
}

func showSkillDetailDialog(owner walk.Form, manager *app.Manager, workDir string, item domain.Ability) error {
	files, err := manager.ListSkillFiles(item.ID)
	if err != nil {
		return err
	}
	detail := &skillDetailWindow{
		manager:   manager,
		workDir:   workDir,
		item:      item,
		fileModel: &skillFileListModel{},
	}
	detail.fileModel.setItems(files)
	if err := detail.create(owner); err != nil {
		return err
	}
	detail.Disposing().Attach(func() {
		detail.closed.Store(true)
		detail.cancelTranslation()
	})
	detail.selectDefaultFile()
	detail.Run()
	return nil
}

func (d *skillDetailWindow) create(owner walk.Form) error {
	title := "Skill 详情 · " + firstNonEmpty(d.item.DisplayName, d.item.Name)
	return Dialog{
		AssignTo:     &d.Dialog,
		Title:        title,
		Size:         Size{Width: 990, Height: 720},
		MinSize:      Size{Width: 760, Height: 560},
		CancelButton: &d.closeButton,
		Font:         Font{Family: "Microsoft YaHei UI", PointSize: 9},
		Background:   SolidColorBrush{Color: atlasPaper},
		Layout:       VBox{Margins: Margins{Left: 12, Top: 10, Right: 12, Bottom: 10}, Spacing: 8},
		Children: []Widget{
			Label{
				Text:      firstNonEmpty(d.item.DisplayName, d.item.Name),
				Font:      Font{Family: "Microsoft YaHei UI", PointSize: 12, Bold: true},
				TextColor: atlasInk,
			},
			HSplitter{
				StretchFactor: 1,
				Children: []Widget{
					ListBox{
						AssignTo:              &d.fileList,
						Model:                 d.fileModel,
						MinSize:               Size{Width: 230, Height: 420},
						OnCurrentIndexChanged: d.loadSelectedFile,
						ToolTipText:           "选择 Skill 文件",
					},
					Composite{
						StretchFactor: 1,
						Layout:        VBox{Margins: Margins{Left: 8, Top: 0, Right: 0, Bottom: 0}, Spacing: 7},
						Children: []Widget{
							Label{
								AssignTo:     &d.pathLabel,
								Text:         "未选择文件",
								Font:         Font{Family: "Cascadia Mono", PointSize: 8},
								TextColor:    atlasMuted,
								EllipsisMode: EllipsisPath,
							},
							Composite{
								Layout: HBox{MarginsZero: true, Spacing: 6},
								Children: []Widget{
									PushButton{AssignTo: &d.sourceButton, Text: "原文", OnClicked: d.showSource},
									PushButton{AssignTo: &d.translationButton, Text: "译文", OnClicked: d.showCachedTranslation},
									PushButton{AssignTo: &d.translateButton, Text: "翻译", OnClicked: d.translateCurrent},
									HSpacer{},
									PushButton{AssignTo: &d.copyButton, Text: "复制", OnClicked: d.copyCurrentText},
									PushButton{AssignTo: &d.openDirButton, Text: "打开目录", OnClicked: d.openDirectory},
								},
							},
							TextEdit{
								AssignTo:      &d.content,
								ReadOnly:      true,
								StretchFactor: 1,
								Font:          Font{Family: "Cascadia Mono", PointSize: 9},
								TextColor:     atlasInk,
								Background:    SolidColorBrush{Color: atlasWhite},
							},
							Label{
								AssignTo:  &d.status,
								Text:      "就绪",
								TextColor: atlasMuted,
								MinSize:   Size{Height: 22},
							},
						},
					},
				},
			},
			Composite{
				Layout: HBox{MarginsZero: true, Spacing: 8},
				Children: []Widget{
					HSpacer{},
					PushButton{
						AssignTo: &d.closeButton,
						Text:     "关闭",
						OnClicked: func() {
							d.Cancel()
						},
					},
				},
			},
		},
	}.Create(owner)
}

func (d *skillDetailWindow) selectDefaultFile() {
	if d.fileList == nil {
		return
	}
	index := d.fileModel.defaultIndex()
	if index < 0 {
		d.showReadError("", fmt.Errorf("没有可显示的 Skill 文件"))
		return
	}
	previous := d.fileList.CurrentIndex()
	if err := d.fileList.SetCurrentIndex(index); err != nil {
		d.showReadError("", err)
		return
	}
	if shouldLoadAfterDefaultSelection(previous, index) {
		d.loadSelectedFile()
	}
}

func shouldLoadAfterDefaultSelection(previous, next int) bool {
	return previous == next
}

func (d *skillDetailWindow) loadSelectedFile() {
	if d.busy || d.fileList == nil {
		return
	}
	file, ok := d.fileModel.current(d.fileList.CurrentIndex())
	if !ok {
		return
	}
	doc, err := d.manager.ReadSkillFile(d.item.ID, file.RelativePath)
	if err != nil {
		d.showReadError(file.RelativePath, err)
		return
	}
	d.current = doc
	// 缓存命中时默认显示中文译文；没有缓存时保持原文，绝不在读取时自动调用 AI。
	d.showingTranslation = showTranslationByDefault(doc)
	d.updateContent()
}

func (d *skillDetailWindow) showReadError(relativePath string, err error) {
	text := "读取失败\r\n\r\n" + err.Error()
	d.current = app.SkillDocumentView{
		SkillID:      d.item.ID,
		RelativePath: relativePath,
		SourceText:   text,
		DisplayText:  text,
		Language:     translate.LanguageUnknown,
	}
	d.showingTranslation = false
	d.updateContent()
	if d.status != nil {
		d.status.SetText("读取失败")
	}
}

func (d *skillDetailWindow) showSource() {
	if d.busy {
		return
	}
	d.showingTranslation = false
	d.updateContent()
}

func (d *skillDetailWindow) showCachedTranslation() {
	if d.busy || !d.current.HasTranslation {
		return
	}
	d.showingTranslation = true
	d.updateContent()
}

func (d *skillDetailWindow) translateCurrent() {
	if d.busy || !d.current.CanTranslate {
		return
	}
	skillID := d.item.ID
	relativePath := d.current.RelativePath
	if strings.TrimSpace(relativePath) == "" {
		return
	}
	d.setBusy(true)
	d.setStatus("翻译中...")
	translator := ai.Translator{WorkDir: d.workDir, Timeout: 2 * time.Minute}
	ctx, cancel := context.WithCancel(context.Background())
	d.translateCancel = cancel

	go func() {
		// 翻译调用走本机 codex，可能持续较久；放到后台执行，避免阻塞 Walk UI 线程。
		doc, err := d.manager.TranslateSkillFile(ctx, translator, skillID, relativePath)
		if d.closed.Load() {
			return
		}
		d.Synchronize(func() {
			if d.closed.Load() || d.IsDisposed() {
				return
			}
			d.translateCancel = nil
			d.setBusy(false)
			if err != nil {
				walk.MsgBox(d, "翻译失败", err.Error(), walk.MsgBoxIconError)
				d.setStatus(skillDetailStatus(d.current))
				return
			}
			d.current = doc
			// 翻译成功后立即显示新缓存，帮助用户确认“重新翻译”已经生效。
			d.showingTranslation = true
			d.updateContent()
			d.setStatus("翻译缓存已更新")
		})
	}()
}

func (d *skillDetailWindow) cancelTranslation() {
	if d.translateCancel != nil {
		d.translateCancel()
		d.translateCancel = nil
	}
}

func (d *skillDetailWindow) copyCurrentText() {
	if d.busy {
		return
	}
	text := skillDetailDisplayText(d.current, d.showingTranslation)
	if strings.TrimSpace(text) == "" {
		return
	}
	if err := walk.Clipboard().SetText(text); err != nil {
		walk.MsgBox(d, "复制失败", err.Error(), walk.MsgBoxIconError)
		return
	}
	d.setStatus("已复制当前内容")
}

func (d *skillDetailWindow) openDirectory() {
	if d.busy || strings.TrimSpace(d.item.Directory) == "" {
		return
	}
	if err := launchDetached("explorer.exe", d.item.Directory); err != nil {
		walk.MsgBox(d, "打开目录失败", err.Error(), walk.MsgBoxIconError)
	}
}

func (d *skillDetailWindow) updateContent() {
	if d.pathLabel != nil {
		d.pathLabel.SetText(firstNonEmpty(d.current.RelativePath, "未选择文件"))
	}
	if d.content != nil {
		d.content.SetText(skillDetailDisplayText(d.current, d.showingTranslation))
	}
	d.setStatus(skillDetailStatus(d.current))
	d.updateButtonState()
}

func (d *skillDetailWindow) setBusy(busy bool) {
	d.busy = busy
	d.updateButtonState()
	if d.fileList != nil {
		d.fileList.SetEnabled(!busy)
	}
	// 按钮状态统一从 current 文档和 busy 推导，避免翻译期间出现“译文可点但内容还未更新”的短暂错觉。
}

func (d *skillDetailWindow) updateButtonState() {
	canUseDoc := strings.TrimSpace(d.current.RelativePath) != ""
	displayText := strings.TrimSpace(skillDetailDisplayText(d.current, d.showingTranslation))
	setButtonText(d.sourceButton, activeText(!d.showingTranslation, "原文"))
	setButtonText(d.translationButton, activeText(d.showingTranslation, "译文"))
	setButtonText(d.translateButton, translateButtonText(d.current))

	for _, button := range []*walk.PushButton{d.sourceButton, d.copyButton, d.openDirButton, d.closeButton} {
		if button != nil {
			button.SetEnabled(!d.busy)
		}
	}
	if d.sourceButton != nil {
		d.sourceButton.SetEnabled(!d.busy && canUseDoc)
	}
	if d.translationButton != nil {
		d.translationButton.SetEnabled(!d.busy && d.current.HasTranslation)
	}
	if d.translateButton != nil {
		d.translateButton.SetVisible(translateActionVisible(d.current))
		d.translateButton.SetEnabled(!d.busy && d.current.CanTranslate)
	}
	if d.copyButton != nil {
		d.copyButton.SetEnabled(!d.busy && displayText != "")
	}
	if d.openDirButton != nil {
		d.openDirButton.SetEnabled(!d.busy && strings.TrimSpace(d.item.Directory) != "")
	}
}

func (d *skillDetailWindow) setStatus(text string) {
	if d.status != nil {
		d.status.SetText(text)
	}
}

func translateButtonText(doc app.SkillDocumentView) string {
	if doc.HasTranslation {
		return "重新翻译"
	}
	return "翻译"
}

func showTranslationByDefault(doc app.SkillDocumentView) bool {
	return doc.HasTranslation
}

func translateActionVisible(doc app.SkillDocumentView) bool {
	return doc.CanTranslate
}

func skillDetailDisplayText(doc app.SkillDocumentView, showTranslation bool) string {
	if showTranslation && doc.HasTranslation && strings.TrimSpace(doc.DisplayText) != "" {
		return doc.DisplayText
	}
	if strings.TrimSpace(doc.SourceText) != "" {
		return doc.SourceText
	}
	return doc.DisplayText
}

func skillDetailStatus(doc app.SkillDocumentView) string {
	if doc.Language == translate.LanguageChinese {
		return "中文，无需翻译"
	}
	if doc.HasTranslation {
		return "已显示缓存译文"
	}
	if doc.CanTranslate {
		if doc.Language == translate.LanguageEnglish {
			return "英文，尚未翻译"
		}
		return "未知语言，尚未翻译"
	}
	return "不可翻译"
}
