//go:build windows

package ui

import (
	"context"
	"fmt"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"codex-atlas/internal/ai"
	"codex-atlas/internal/app"
	"codex-atlas/internal/domain"

	"github.com/lxn/walk"
	. "github.com/lxn/walk/declarative"
	"github.com/lxn/win"
)

type atlasWindow struct {
	*walk.MainWindow
	manager *app.Manager
	workDir string

	searchEdit *walk.LineEdit
	sortBox    *walk.ComboBox
	listBox    *walk.ListBox
	listModel  *abilityListModel

	skillButton  *walk.PushButton
	pluginButton *walk.PushButton
	toolButton   *walk.PushButton
	appButton    *walk.PushButton

	commonButton *walk.PushButton
	debugButton  *walk.PushButton
	docButton    *walk.PushButton
	designButton *walk.PushButton
	hiddenButton *walk.PushButton

	moreButton *walk.PushButton
	editButton *walk.PushButton
	copyButton *walk.PushButton
	morePanel  *walk.Composite
	editPanel  *walk.Composite

	refreshScanButton  *walk.PushButton
	refreshStatsButton *walk.PushButton
	refreshAllButton   *walk.PushButton
	aiButton           *walk.PushButton
	saveButton         *walk.PushButton
	cancelEditButton   *walk.PushButton

	detailNameLabel    *walk.Label
	detailPathLabel    *walk.Label
	detailKindLabel    *walk.Label
	detailSummaryLabel *walk.Label
	templatePreview    *walk.TextEdit

	aliasEdit    *walk.LineEdit
	tagsEdit     *walk.LineEdit
	noteEdit     *walk.TextEdit
	templateEdit *walk.LineEdit
	favoriteBox  *walk.CheckBox
	hiddenBox    *walk.CheckBox
	statusItem   *walk.StatusBarItem

	currentKind domain.AbilityKind
	quickFilter string
	panelMode   string
	busy        bool
}

const (
	panelDefault = ""
	panelMore    = "more"
	panelEdit    = "edit"
)

func Run(manager *app.Manager, workDir string) error {
	if err := manager.Load(); err != nil {
		return err
	}
	w := &atlasWindow{
		manager:     manager,
		workDir:     workDir,
		listModel:   &abilityListModel{},
		currentKind: domain.KindSkill,
	}
	if len(manager.Scan.Abilities) == 0 {
		// 首次启动没有能力清单缓存时只同步扫描清单；使用统计由缓存提供，或由用户手动刷新。
		_ = manager.RefreshScan(context.Background())
	}
	w.listModel.setItems(manager.Abilities(w.currentKind, "", false))

	if err := w.create(); err != nil {
		return err
	}
	w.refreshList()
	w.placeAsSidebar()
	stopHotkey := startHotkey(func() {
		w.Synchronize(func() {
			w.SetVisible(!w.Visible())
			if w.Visible() {
				w.BringToTop()
			}
		})
	}, func(err error) {
		w.Synchronize(func() {
			w.setStatus(err.Error())
		})
	})
	w.Closing().Attach(func(canceled *bool, reason walk.CloseReason) {
		stopHotkey()
	})
	w.Run()
	return nil
}

func (w *atlasWindow) create() error {
	sortItems := []string{"默认", "名称", "使用次数", "最近使用"}
	return (MainWindow{
		AssignTo: &w.MainWindow,
		Title:    "Codex Atlas",
		Bounds:   Rectangle{X: 80, Y: 40, Width: 460, Height: 640},
		MinSize:  Size{420, 560},
		Layout:   VBox{Margins: Margins{Left: 12, Top: 12, Right: 12, Bottom: 8}, Spacing: 8},
		Font:     Font{Family: "Microsoft YaHei UI", PointSize: 9},
		Background: SolidColorBrush{
			Color: atlasPaper,
		},
		Children: []Widget{
			Composite{
				Background: SolidColorBrush{Color: atlasPaper2},
				Layout:     VBox{Margins: Margins{Left: 0, Top: 0, Right: 0, Bottom: 0}, Spacing: 5},
				Children: []Widget{
					Composite{
						Background: SolidColorBrush{Color: atlasPaper2},
						Layout:     HBox{MarginsZero: true, Spacing: 8},
						Children: []Widget{
							Label{
								Text:          "CA",
								MinSize:       Size{Width: 28, Height: 24},
								TextAlignment: AlignCenter,
								Font:          Font{Family: "Microsoft YaHei UI", PointSize: 8, Bold: true},
								TextColor:     atlasPaper2,
								Background:    SolidColorBrush{Color: atlasInk},
							},
							Label{
								Text:      "Codex Atlas",
								Font:      Font{Family: "Microsoft YaHei UI", PointSize: 9, Bold: true},
								TextColor: atlasInk,
							},
							HSpacer{},
							Label{
								Text:          "Ctrl Alt Space",
								TextAlignment: AlignCenter,
								MinSize:       Size{Width: 94, Height: 22},
								Font:          Font{Family: "Cascadia Mono", PointSize: 8},
								TextColor:     atlasMuted,
								Background:    SolidColorBrush{Color: atlasWhite},
							},
						},
					},
					Label{
						Text:      "Codex 能力索引",
						Font:      Font{Family: "Microsoft YaHei UI", PointSize: 14, Bold: true},
						TextColor: atlasInk,
					},
					Label{
						Text:      "描述你要做什么，然后把合适的 Codex 能力带回对话里。",
						Font:      Font{Family: "Microsoft YaHei UI", PointSize: 8},
						TextColor: atlasMuted,
					},
				},
			},
			Composite{
				Background: SolidColorBrush{Color: atlasPaper},
				Layout:     HBox{MarginsZero: true, Spacing: 6},
				Children: []Widget{
					Label{
						Text:          ">",
						MinSize:       Size{Width: 22, Height: 30},
						TextAlignment: AlignCenter,
						Font:          Font{Family: "Cascadia Mono", PointSize: 11, Bold: true},
						TextColor:     atlasClay,
					},
					LineEdit{
						AssignTo:      &w.searchEdit,
						CueBanner:     "搜索能力、标签、用途或备注",
						StretchFactor: 1,
						OnTextChanged: w.refreshList,
					},
				},
			},
			Composite{
				Background: SolidColorBrush{Color: atlasPaper},
				Layout:     HBox{MarginsZero: true, Spacing: 5},
				Children: []Widget{
					PushButton{AssignTo: &w.skillButton, Text: "Skills", StretchFactor: 1, OnClicked: func() { w.setKind(domain.KindSkill) }},
					PushButton{AssignTo: &w.pluginButton, Text: "Plugins", StretchFactor: 1, OnClicked: func() { w.setKind(domain.KindPlugin) }},
					PushButton{AssignTo: &w.toolButton, Text: "Tools", StretchFactor: 1, OnClicked: func() { w.setKind(domain.KindTool) }},
					PushButton{AssignTo: &w.appButton, Text: "Apps", StretchFactor: 1, OnClicked: func() { w.setKind(domain.KindApp) }},
				},
			},
			Composite{
				Background: SolidColorBrush{Color: atlasPaper},
				Layout:     HBox{MarginsZero: true, Spacing: 5},
				Children: []Widget{
					PushButton{AssignTo: &w.commonButton, Text: "常用", OnClicked: func() { w.toggleQuickFilter("common") }},
					PushButton{AssignTo: &w.debugButton, Text: "调试", OnClicked: func() { w.toggleQuickFilter("调试") }},
					PushButton{AssignTo: &w.docButton, Text: "写文档", OnClicked: func() { w.toggleQuickFilter("写文档") }},
					PushButton{AssignTo: &w.designButton, Text: "设计", OnClicked: func() { w.toggleQuickFilter("设计") }},
					PushButton{AssignTo: &w.hiddenButton, Text: "隐藏", OnClicked: func() { w.toggleQuickFilter("hidden") }},
				},
			},
			ListBox{
				AssignTo:              &w.listBox,
				Background:            SolidColorBrush{Color: atlasPaper},
				DoubleBuffering:       true,
				Model:                 w.listModel,
				StretchFactor:         1,
				OnCurrentIndexChanged: w.updateDetail,
				OnItemActivated:       w.copyTemplate,
				OnMouseMove:           w.showHoverSummary,
				ToolTipText:           "双击复制第一个调用模板",
			},
			Composite{
				Background: SolidColorBrush{Color: atlasPaper2},
				Border:     true,
				Layout:     VBox{Margins: Margins{Left: 9, Top: 8, Right: 9, Bottom: 8}, Spacing: 6},
				Children: []Widget{
					Composite{
						Background: SolidColorBrush{Color: atlasPaper2},
						Layout:     HBox{MarginsZero: true, Spacing: 8},
						Children: []Widget{
							Composite{
								Background:    SolidColorBrush{Color: atlasPaper2},
								StretchFactor: 1,
								Layout:        VBox{MarginsZero: true, Spacing: 2},
								Children: []Widget{
									Label{AssignTo: &w.detailNameLabel, Text: "未选择能力", Font: Font{Family: "Microsoft YaHei UI", PointSize: 10, Bold: true}, TextColor: atlasInk},
									Label{AssignTo: &w.detailPathLabel, Text: "", Font: Font{Family: "Cascadia Mono", PointSize: 8}, TextColor: atlasMuted, EllipsisMode: EllipsisPath},
								},
							},
							Label{AssignTo: &w.detailKindLabel, Text: "skill", MinSize: Size{Width: 48, Height: 23}, TextAlignment: AlignCenter, TextColor: atlasPaper2, Background: SolidColorBrush{Color: atlasClay}},
						},
					},
					Label{
						AssignTo:      &w.detailSummaryLabel,
						Text:          "选择一个能力查看摘要。",
						Font:          Font{Family: "Microsoft YaHei UI", PointSize: 8},
						TextColor:     atlasMuted,
						EllipsisMode:  EllipsisEnd,
						MinSize:       Size{Height: 18},
						StretchFactor: 0,
					},
					TextEdit{
						AssignTo:  &w.templatePreview,
						ReadOnly:  true,
						MinSize:   Size{Height: 46},
						MaxSize:   Size{Height: 58},
						Font:      Font{Family: "Cascadia Mono", PointSize: 8},
						TextColor: atlasPaper2,
						Background: SolidColorBrush{
							Color: atlasInk,
						},
					},
					Composite{
						Background: SolidColorBrush{Color: atlasPaper2},
						Layout:     HBox{MarginsZero: true, Spacing: 6},
						Children: []Widget{
							PushButton{AssignTo: &w.copyButton, Text: "复制模板", StretchFactor: 1, OnClicked: w.copyTemplate},
							PushButton{AssignTo: &w.moreButton, Text: "更多", StretchFactor: 1, OnClicked: func() { w.togglePanel(panelMore) }},
							PushButton{AssignTo: &w.editButton, Text: "编辑", StretchFactor: 1, OnClicked: func() { w.togglePanel(panelEdit) }},
						},
					},
					Composite{
						AssignTo:   &w.morePanel,
						Visible:    false,
						Border:     true,
						Background: SolidColorBrush{Color: atlasWhite},
						Layout:     VBox{Margins: Margins{Left: 8, Top: 8, Right: 8, Bottom: 8}, Spacing: 6},
						Children: []Widget{
							Label{Text: "文件与路径", Font: Font{Family: "Microsoft YaHei UI", PointSize: 8, Bold: true}, TextColor: atlasMuted},
							Composite{
								Layout: HBox{MarginsZero: true, Spacing: 6},
								Children: []Widget{
									PushButton{Text: "打开文件", StretchFactor: 1, OnClicked: w.openSourceFile},
									PushButton{Text: "打开目录", StretchFactor: 1, OnClicked: w.openSourceDir},
									PushButton{Text: "复制路径", StretchFactor: 1, OnClicked: w.copyPath},
								},
							},
							Composite{
								Layout: HBox{MarginsZero: true, Spacing: 6},
								Children: []Widget{
									PushButton{Text: "完整详情", StretchFactor: 1, OnClicked: w.showFullDetail},
								},
							},
							Label{Text: "刷新与整理", Font: Font{Family: "Microsoft YaHei UI", PointSize: 8, Bold: true}, TextColor: atlasMuted},
							Composite{
								Layout: HBox{MarginsZero: true, Spacing: 6},
								Children: []Widget{
									PushButton{AssignTo: &w.refreshScanButton, Text: "刷新清单", StretchFactor: 1, OnClicked: func() {
										w.runAsync("刷新能力清单", func() error { return w.manager.RefreshScan(context.Background()) })
									}},
									PushButton{AssignTo: &w.refreshStatsButton, Text: "刷新统计", StretchFactor: 1, OnClicked: func() { w.runAsync("刷新使用统计", w.manager.RefreshStats) }},
									PushButton{AssignTo: &w.aiButton, Text: "AI 整理", StretchFactor: 1, OnClicked: w.organizeAI},
								},
							},
							Composite{
								Layout: HBox{MarginsZero: true, Spacing: 6},
								Children: []Widget{
									Label{Text: "排序", MinSize: Size{Width: 34}, TextColor: atlasMuted},
									ComboBox{
										AssignTo:              &w.sortBox,
										Model:                 sortItems,
										CurrentIndex:          0,
										StretchFactor:         1,
										OnCurrentIndexChanged: w.refreshList,
									},
									PushButton{AssignTo: &w.refreshAllButton, Text: "全部刷新", StretchFactor: 1, OnClicked: w.refreshAll},
								},
							},
						},
					},
					Composite{
						AssignTo:   &w.editPanel,
						Visible:    false,
						Border:     true,
						Background: SolidColorBrush{Color: atlasWhite},
						Layout:     VBox{Margins: Margins{Left: 8, Top: 8, Right: 8, Bottom: 8}, Spacing: 6},
						Children: []Widget{
							Composite{
								Layout: HBox{MarginsZero: true, Spacing: 6},
								Children: []Widget{
									LineEdit{AssignTo: &w.aliasEdit, CueBanner: "别名", StretchFactor: 1},
									LineEdit{AssignTo: &w.tagsEdit, CueBanner: "标签，用逗号分隔", StretchFactor: 1},
								},
							},
							LineEdit{AssignTo: &w.templateEdit, CueBanner: "自定义调用模板"},
							TextEdit{AssignTo: &w.noteEdit, MinSize: Size{Height: 48}},
							Composite{
								Layout: HBox{MarginsZero: true, Spacing: 8},
								Children: []Widget{
									CheckBox{AssignTo: &w.favoriteBox, Text: "收藏"},
									CheckBox{AssignTo: &w.hiddenBox, Text: "隐藏"},
									HSpacer{},
									PushButton{AssignTo: &w.cancelEditButton, Text: "取消", OnClicked: func() {
										w.updateDetail()
										w.setPanel(panelDefault)
									}},
									PushButton{AssignTo: &w.saveButton, Text: "保存整理", OnClicked: w.saveUserData},
								},
							},
						},
					},
				},
			},
		},
		StatusBarItems: []StatusBarItem{{AssignTo: &w.statusItem, Text: "就绪", Width: 420}},
	}).Create()
}

func (w *atlasWindow) showHoverSummary(x, y int, _ walk.MouseButton) {
	if w.listBox == nil {
		return
	}
	result := uint32(w.listBox.SendMessage(win.LB_ITEMFROMPOINT, 0, uintptr(win.MAKELONG(uint16(x), uint16(y)))))
	if win.HIWORD(result) != 0 {
		return
	}
	index := int(win.LOWORD(result))
	item, ok := w.listModel.current(index)
	if !ok {
		return
	}
	summary := firstNonEmpty(item.Summary, item.Description, item.Name)
	if len([]rune(summary)) > 90 {
		summary = string([]rune(summary)[:90]) + "..."
	}
	w.setStatus(item.DisplayName + " - " + summary)
}

func (w *atlasWindow) refreshList() {
	if w.busy {
		return
	}
	query := ""
	if w.searchEdit != nil {
		query = w.searchEdit.Text()
	}
	items := w.manager.Abilities(w.currentKind, query, w.quickFilter == "hidden")
	items = w.applyQuickFilter(items)
	w.applySort(items)
	w.listModel.setItems(items)
	if w.listBox != nil && len(w.listModel.items) > 0 {
		_ = w.listBox.SetCurrentIndex(0)
	}
	w.updateDetail()
	w.setStatus(fmt.Sprintf("%s：%d 项", w.currentKind, len(w.listModel.items)))
	w.updateChromeState()
}

func (w *atlasWindow) setKind(kind domain.AbilityKind) {
	w.currentKind = kind
	w.refreshList()
}

func (w *atlasWindow) toggleQuickFilter(filter string) {
	if w.quickFilter == filter {
		w.quickFilter = ""
	} else {
		w.quickFilter = filter
	}
	w.refreshList()
}

func (w *atlasWindow) togglePanel(mode string) {
	if w.panelMode == mode {
		w.setPanel(panelDefault)
		return
	}
	w.setPanel(mode)
}

func (w *atlasWindow) setPanel(mode string) {
	w.panelMode = mode
	if w.morePanel != nil {
		w.morePanel.SetVisible(mode == panelMore)
	}
	if w.editPanel != nil {
		w.editPanel.SetVisible(mode == panelEdit)
	}
	w.updateChromeState()
}

func (w *atlasWindow) selected() (domain.Ability, bool) {
	if w.listBox == nil {
		return domain.Ability{}, false
	}
	return w.listModel.current(w.listBox.CurrentIndex())
}

func (w *atlasWindow) applyQuickFilter(items []domain.Ability) []domain.Ability {
	switch w.quickFilter {
	case "":
		return items
	case "common":
		return filterAbilities(items, func(item domain.Ability) bool {
			return item.User.Favorite || item.Stats.UsageCount > 0
		})
	case "hidden":
		return filterAbilities(items, func(item domain.Ability) bool {
			return item.User.Hidden
		})
	default:
		token := strings.ToLower(w.quickFilter)
		return filterAbilities(items, func(item domain.Ability) bool {
			text := strings.ToLower(strings.Join([]string{
				item.DisplayName,
				item.Name,
				item.Summary,
				item.Description,
				strings.Join(item.Tags, " "),
				strings.Join(item.UseScenarios, " "),
				item.User.Note,
			}, " "))
			return strings.Contains(text, token)
		})
	}
}

func filterAbilities(items []domain.Ability, keep func(domain.Ability) bool) []domain.Ability {
	out := items[:0]
	for _, item := range items {
		if keep(item) {
			out = append(out, item)
		}
	}
	return out
}

func (w *atlasWindow) updateDetail() {
	item, ok := w.selected()
	if !ok {
		w.setDetailText("没有可显示的能力", "", "", "请点击“更多 / 刷新清单”。", "")
		w.setEditFields(domain.Ability{})
		return
	}
	template := ""
	if len(item.CallTemplates) > 0 {
		template = item.CallTemplates[0].Text
	}
	w.setDetailText(item.DisplayName, compactPath(item.SourcePath), string(item.Kind), detailSummary(item), template)
	w.setEditFields(item)
}

func (w *atlasWindow) setDetailText(name, path, kind, summary, template string) {
	if w.detailNameLabel != nil {
		w.detailNameLabel.SetText(firstNonEmpty(name, "未选择能力"))
	}
	if w.detailPathLabel != nil {
		w.detailPathLabel.SetText(path)
	}
	if w.detailKindLabel != nil {
		w.detailKindLabel.SetText(firstNonEmpty(kind, "skill"))
	}
	if w.detailSummaryLabel != nil {
		w.detailSummaryLabel.SetText(summary)
	}
	if w.templatePreview != nil {
		w.templatePreview.SetText(template)
	}
}

func (w *atlasWindow) setEditFields(item domain.Ability) {
	if w.aliasEdit != nil {
		w.aliasEdit.SetText(item.User.Alias)
	}
	if w.tagsEdit != nil {
		w.tagsEdit.SetText(strings.Join(item.User.Tags, ", "))
	}
	if w.templateEdit != nil {
		if len(item.User.CustomTemplates) > 0 {
			w.templateEdit.SetText(item.User.CustomTemplates[0].Text)
		} else {
			w.templateEdit.SetText("")
		}
	}
	if w.noteEdit != nil {
		w.noteEdit.SetText(item.User.Note)
	}
	if w.favoriteBox != nil {
		w.favoriteBox.SetChecked(item.User.Favorite)
	}
	if w.hiddenBox != nil {
		w.hiddenBox.SetChecked(item.User.Hidden)
	}
}

func detailSummary(item domain.Ability) string {
	parts := []string{firstNonEmpty(item.Summary, item.Description, "暂无摘要。")}
	if item.Kind == domain.KindSkill {
		parts = append(parts, usageLabel(item.Stats))
	}
	if len(item.Tags) > 0 {
		parts = append(parts, "标签: "+strings.Join(item.Tags, " / "))
	}
	return strings.Join(parts, "  ")
}

func compactPath(path string) string {
	path = strings.TrimSpace(filepath.ToSlash(path))
	if path == "" {
		return ""
	}
	parts := strings.Split(path, "/")
	if len(parts) <= 3 {
		return path
	}
	return strings.Join(parts[len(parts)-3:], "/")
}

func renderDetail(item domain.Ability) string {
	lines := []string{
		item.DisplayName,
		"真实名称: " + item.Name,
		"类型: " + string(item.Kind),
		"来源: " + item.Source,
		"状态: " + item.Status,
		"",
		"摘要:",
		firstNonEmpty(item.Summary, item.Description),
	}
	if item.Plugin != "" {
		lines = append(lines, "", "插件: "+item.Plugin)
	}
	if len(item.Tags) > 0 {
		lines = append(lines, "", "标签: "+strings.Join(item.Tags, " / "))
	}
	if item.Description != "" && item.Description != item.Summary {
		lines = append(lines, "", "原始描述:", item.Description)
	}
	if len(item.UseScenarios) > 0 {
		lines = append(lines, "", "适用场景:", "- "+strings.Join(item.UseScenarios, "\r\n- "))
	}
	if len(item.AvoidScenarios) > 0 {
		lines = append(lines, "", "不适用场景:", "- "+strings.Join(item.AvoidScenarios, "\r\n- "))
	}
	if len(item.AI.Similar) > 0 {
		lines = append(lines, "", "相似能力: "+strings.Join(item.AI.Similar, " / "))
	}
	if len(item.ChildIDs) > 0 {
		lines = append(lines, "", fmt.Sprintf("包含能力: %d 个", len(item.ChildIDs)), strings.Join(item.ChildIDs, "\r\n"))
	}
	if len(item.CallTemplates) > 0 {
		var templates []string
		for _, tpl := range item.CallTemplates {
			templates = append(templates, tpl.Title+": "+tpl.Text)
		}
		lines = append(lines, "", "调用模板:", strings.Join(templates, "\r\n"))
	}
	if item.Kind == domain.KindSkill {
		lines = append(lines, "", "统计: "+usageDetailLabel(item.Stats))
		if !item.Stats.LastUsedAt.IsZero() {
			lines = append(lines, "最近使用: "+item.Stats.LastUsedAt.Format("2006-01-02 15:04"))
		}
	}
	lines = append(lines, "", "来源路径:", item.SourcePath)
	return strings.Join(lines, "\r\n")
}

func (w *atlasWindow) saveUserData() {
	if w.busy {
		return
	}
	item, ok := w.selected()
	if !ok {
		return
	}
	err := w.manager.UpdateUser(item.ID, func(user *domain.UserData) {
		user.Alias = strings.TrimSpace(w.aliasEdit.Text())
		user.Tags = splitTags(w.tagsEdit.Text())
		user.Note = strings.TrimSpace(w.noteEdit.Text())
		user.Favorite = w.favoriteBox.Checked()
		user.Hidden = w.hiddenBox.Checked()
		template := strings.TrimSpace(w.templateEdit.Text())
		if template != "" {
			user.CustomTemplates = []domain.CallTemplate{{Title: "我的模板", Text: template}}
		} else {
			user.CustomTemplates = nil
		}
	})
	if err != nil {
		w.showError("保存失败", err)
		return
	}
	w.refreshList()
	w.setPanel(panelDefault)
	w.setStatus("已保存个人整理")
}

func (w *atlasWindow) copyTemplate() {
	item, ok := w.selected()
	if !ok || len(item.CallTemplates) == 0 {
		return
	}
	if err := walk.Clipboard().SetText(item.CallTemplates[0].Text); err != nil {
		w.showError("复制模板失败", err)
		return
	}
	w.setStatus("已复制调用模板")
}

func (w *atlasWindow) copyPath() {
	item, ok := w.selected()
	if !ok {
		return
	}
	if err := walk.Clipboard().SetText(item.SourcePath); err != nil {
		w.showError("复制路径失败", err)
		return
	}
	w.setStatus("已复制来源路径")
}

func (w *atlasWindow) showFullDetail() {
	item, ok := w.selected()
	if !ok {
		return
	}
	walk.MsgBox(w, "完整详情", renderDetail(item), walk.MsgBoxOK)
}

func (w *atlasWindow) openSourceFile() {
	item, ok := w.selected()
	if !ok || item.SourcePath == "" {
		return
	}
	if err := launchDetached("rundll32.exe", "url.dll,FileProtocolHandler", item.SourcePath); err != nil {
		w.showError("打开文件失败", err)
	}
}

func (w *atlasWindow) openSourceDir() {
	item, ok := w.selected()
	if !ok || item.Directory == "" {
		return
	}
	if err := launchDetached("explorer.exe", item.Directory); err != nil {
		w.showError("打开目录失败", err)
	}
}

func launchDetached(name string, args ...string) error {
	cmd := exec.Command(name, args...)
	if err := cmd.Start(); err != nil {
		return err
	}
	// 这里启动的是 explorer/rundll32 等系统程序，交给系统继续处理后立即释放进程句柄，
	// 避免长期驻留的小工具因为频繁打开文件而积累无用资源。
	return cmd.Process.Release()
}

func (w *atlasWindow) refreshAll() {
	w.runAsync("全部刷新", func() error {
		if err := w.manager.RefreshScan(context.Background()); err != nil {
			return err
		}
		return w.manager.RefreshStats()
	})
}

func (w *atlasWindow) organizeAI() {
	if w.busy {
		return
	}
	outputPath := filepath.Join(w.workDir, ".tmp", "ai-output.json")
	organizer := ai.Organizer{WorkDir: w.workDir, OutputPath: outputPath, Timeout: 2 * time.Minute}
	// 对当前 UI 可见列表取快照，让“AI 整理”严格作用于搜索和快捷筛选后的结果。
	items := w.currentVisibleItems()
	w.runAsync("AI 整理", func() error {
		return w.manager.OrganizeAIItems(context.Background(), organizer, items)
	})
}

func (w *atlasWindow) currentVisibleItems() []domain.Ability {
	items := make([]domain.Ability, len(w.listModel.items))
	copy(items, w.listModel.items)
	return items
}

func (w *atlasWindow) runAsync(label string, action func() error) {
	if w.busy {
		w.setStatus("已有任务正在运行")
		return
	}
	w.setBusy(true)
	w.setStatus(label + "中...")
	go func() {
		err := action()
		w.Synchronize(func() {
			w.setBusy(false)
			if err != nil {
				w.showError(label+"失败", err)
				w.setStatus(label + "失败")
				return
			}
			w.refreshList()
			w.setStatus(label + "完成")
		})
	}()
}

func (w *atlasWindow) setBusy(busy bool) {
	w.busy = busy
	for _, button := range []*walk.PushButton{
		w.skillButton,
		w.pluginButton,
		w.toolButton,
		w.appButton,
		w.commonButton,
		w.debugButton,
		w.docButton,
		w.designButton,
		w.hiddenButton,
		w.moreButton,
		w.editButton,
		w.copyButton,
		w.refreshScanButton,
		w.refreshStatsButton,
		w.refreshAllButton,
		w.aiButton,
		w.saveButton,
		w.cancelEditButton,
	} {
		if button != nil {
			button.SetEnabled(!busy)
		}
	}
	if w.searchEdit != nil {
		w.searchEdit.SetEnabled(!busy)
	}
	if w.listBox != nil {
		w.listBox.SetEnabled(!busy)
	}
	// 异步刷新/AI 整理期间禁用会改列表状态的控件，避免用户点击后列表延迟变化造成误解。
}

func (w *atlasWindow) placeAsSidebar() {
	width := int32(460)
	screenHeight := win.GetSystemMetrics(win.SM_CYSCREEN)
	height := int32(640)
	if screenHeight-80 < height {
		height = screenHeight - 80
	}
	if height < 560 {
		height = 560
	}
	x := win.GetSystemMetrics(win.SM_CXSCREEN) - width - 12
	win.SetWindowPos(w.Handle(), win.HWND_TOPMOST, x, 40, width, height, win.SWP_SHOWWINDOW)
}

func (w *atlasWindow) setStatus(text string) {
	if w.statusItem != nil {
		w.statusItem.SetText(text)
	}
}

func (w *atlasWindow) showError(title string, err error) {
	walk.MsgBox(w, title, err.Error(), walk.MsgBoxIconError)
}

func splitTags(text string) []string {
	parts := strings.FieldsFunc(text, func(r rune) bool {
		return r == ',' || r == '，' || r == ';' || r == '；' || r == '/'
	})
	var tags []string
	seen := map[string]bool{}
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" || seen[part] {
			continue
		}
		seen[part] = true
		tags = append(tags, part)
	}
	return tags
}

func (w *atlasWindow) applySort(items []domain.Ability) {
	if w.sortBox == nil {
		return
	}
	switch w.sortBox.CurrentIndex() {
	case 1:
		sort.SliceStable(items, func(i, j int) bool {
			return strings.ToLower(items[i].DisplayName) < strings.ToLower(items[j].DisplayName)
		})
	case 2:
		sort.SliceStable(items, func(i, j int) bool {
			return usageSortValue(items[i].Stats) > usageSortValue(items[j].Stats)
		})
	case 3:
		sort.SliceStable(items, func(i, j int) bool {
			return items[i].Stats.LastUsedAt.After(items[j].Stats.LastUsedAt)
		})
	}
}

func (w *atlasWindow) updateChromeState() {
	setButtonText(w.skillButton, activeText(w.currentKind == domain.KindSkill, "Skills"))
	setButtonText(w.pluginButton, activeText(w.currentKind == domain.KindPlugin, "Plugins"))
	setButtonText(w.toolButton, activeText(w.currentKind == domain.KindTool, "Tools"))
	setButtonText(w.appButton, activeText(w.currentKind == domain.KindApp, "Apps"))

	setButtonText(w.commonButton, activeText(w.quickFilter == "common", "常用"))
	setButtonText(w.debugButton, activeText(w.quickFilter == "调试", "调试"))
	setButtonText(w.docButton, activeText(w.quickFilter == "写文档", "写文档"))
	setButtonText(w.designButton, activeText(w.quickFilter == "设计", "设计"))
	setButtonText(w.hiddenButton, activeText(w.quickFilter == "hidden", "隐藏"))

	setButtonText(w.moreButton, activeText(w.panelMode == panelMore, "更多"))
	setButtonText(w.editButton, activeText(w.panelMode == panelEdit, "编辑"))
}

func setButtonText(button *walk.PushButton, text string) {
	if button != nil {
		button.SetText(text)
	}
}

func activeText(active bool, text string) string {
	if active {
		return "● " + text
	}
	return text
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}
