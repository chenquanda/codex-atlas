const summaryItems = [
  { label: "技能", value: "0", tone: "green" },
  { label: "插件", value: "0", tone: "blue" },
  { label: "待扫描", value: "本机", tone: "yellow" }
];

const shellRows = [
  { name: "技能索引", meta: "等待扫描 Codex 能力目录", state: "就绪" },
  { name: "详情阅读器", meta: "后续任务接入 Skill 文件浏览", state: "预留" },
  { name: "翻译缓存", meta: "项目内 data 目录保存用户数据", state: "预留" }
];

export default function App() {
  return (
    <main className="atlas-shell" aria-label="Codex Atlas">
      <section className="atlas-panel">
        <header className="atlas-header">
          <div>
            <p className="atlas-kicker">Local capability index</p>
            <h1>Codex Atlas</h1>
          </div>
          <span className="atlas-status">Tauri v2</span>
        </header>

        <div className="atlas-toolbar" aria-label="能力概览">
          {summaryItems.map((item) => (
            <div className={`atlas-metric atlas-metric--${item.tone}`} key={item.label}>
              <span>{item.label}</span>
              <strong>{item.value}</strong>
            </div>
          ))}
        </div>

        <section className="atlas-workspace" aria-label="工作区">
          <div className="atlas-list">
            <div className="atlas-list-head">
              <span>模块</span>
              <span>状态</span>
            </div>
            {shellRows.map((row) => (
              <article className="atlas-row" key={row.name}>
                <div>
                  <h2>{row.name}</h2>
                  <p>{row.meta}</p>
                </div>
                <span>{row.state}</span>
              </article>
            ))}
          </div>

          <aside className="atlas-side" aria-label="当前阶段">
            <span className="atlas-side-label">当前阶段</span>
            <strong>项目骨架</strong>
            <p>React 入口、Tauri 配置和 Rust 命令层已经准备好承接后续扫描与统计功能。</p>
          </aside>
        </section>
      </section>
    </main>
  );
}
