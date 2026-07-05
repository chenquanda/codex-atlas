import { useEffect, useMemo, useRef, useState } from "react";
import type {
  Ability,
  AtlasApi,
  SkillDetailWindowInfo,
  SkillFileContent,
  SkillFileEntry,
  SkillFileList,
  TranslationState
} from "../api/atlasApi";
import {
  formatUsageLabel,
  getAbilityName,
  getAbilitySummary,
  getAbilityTags
} from "../lib/abilityFormatting";
import { detectContentLanguage } from "../lib/language";

interface DetailWindowProps {
  ability: Ability;
  api: Pick<
    AtlasApi,
    "listSkillFiles" | "readSkillFile" | "getTranslationState" | "translateSkillFile"
  >;
  detailInfo?: SkillDetailWindowInfo | null;
  onClose: () => void;
}

export function DetailWindow({ ability, api, detailInfo = null, onClose }: DetailWindowProps) {
  const [fileList, setFileList] = useState<SkillFileList | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [content, setContent] = useState<SkillFileContent | null>(null);
  const [loading, setLoading] = useState(true);
  const [contentLoading, setContentLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [contentError, setContentError] = useState<string | null>(null);
  const [translationState, setTranslationState] = useState<TranslationState | null>(null);
  const [translationText, setTranslationText] = useState<string | null>(null);
  const [translationChecking, setTranslationChecking] = useState(false);
  const [translationLoading, setTranslationLoading] = useState(false);
  const [translationError, setTranslationError] = useState<string | null>(null);
  const readRequestSequence = useRef(0);
  const translationInFlight = useRef(false);

  const name = getAbilityName(ability);
  const summary = getAbilitySummary(ability);
  const tags = getAbilityTags(ability);
  const usage = formatUsageLabel(ability.stats.usage_count);

  useEffect(() => {
    let active = true;
    const requestId = ++readRequestSequence.current;

    async function loadDetail() {
      setLoading(true);
      setError(null);
      setContent(null);
      setSelectedPath(null);
      setFileList(null);
      resetTranslationState();

      try {
        const nextFiles = await api.listSkillFiles(ability.id);
        if (!active) {
          return;
        }

        setFileList(nextFiles);
        setLoading(false);

        const firstFile = nextFiles.files[0] ?? null;
        if (!firstFile) {
          setSelectedPath(null);
          setContent(null);
          return;
        }

        setSelectedPath(firstFile.relative_path);
        setContent(null);
        setContentLoading(true);
        setContentError(null);
        const firstContent = await api.readSkillFile(ability.id, firstFile.relative_path);
        if (!active || requestId !== readRequestSequence.current) {
          return;
        }
        setContent(firstContent);
        setContentError(null);
        setContentLoading(false);
        await loadTranslationState(firstFile.relative_path, requestId, () => active);
      } catch (reason) {
        if (active && requestId === readRequestSequence.current) {
          setError(errorMessage(reason));
        }
      } finally {
        if (active && requestId === readRequestSequence.current) {
          setLoading(false);
          setContentLoading(false);
        }
      }
    }

    loadDetail();

    return () => {
      active = false;
    };
  }, [ability.id, api]);

  const selectedEntry = useMemo(
    () => fileList?.files.find((entry) => entry.relative_path === selectedPath) ?? null,
    [fileList, selectedPath]
  );
  const visibleContent = content?.relative_path === selectedPath ? content : null;
  const visibleTranslation =
    translationState?.relative_path === selectedPath ? translationText : null;
  const displayedContent = visibleTranslation ?? visibleContent?.content ?? null;
  const contentLanguage = visibleTranslation
    ? "Chinese"
    : visibleContent
      ? detectContentLanguage(visibleContent.content)
      : "Other";
  const showTranslationAction =
    visibleContent !== null &&
    translationState?.relative_path === selectedPath &&
    translationState.show_translate_action &&
    visibleTranslation === null;
  const rootPath = detailInfo?.root_path ?? fileList?.root_path ?? ability.path ?? ability.id;
  const languageNote = translationChecking
    ? "正在检查缓存"
    : visibleTranslation
      ? translationState?.cached_translation
        ? "缓存命中"
        : "手动翻译"
      : contentLanguage === "Chinese"
        ? "隐藏翻译控件"
        : "仅保留手动翻译入口";

  async function handleSelectFile(entry: SkillFileEntry) {
    const requestId = ++readRequestSequence.current;
    setSelectedPath(entry.relative_path);
    setContent(null);
    setContentLoading(true);
    setContentError(null);
    resetTranslationState();

    try {
      const nextContent = await api.readSkillFile(ability.id, entry.relative_path);
      // 文件读取可能乱序返回；只有最新选择的请求可以写入阅读器状态。
      if (requestId !== readRequestSequence.current) {
        return;
      }
      setContent(nextContent);
      setContentLoading(false);
      await loadTranslationState(entry.relative_path, requestId);
    } catch (reason) {
      if (requestId !== readRequestSequence.current) {
        return;
      }
      setContent(null);
      setContentError(errorMessage(reason));
    } finally {
      if (requestId === readRequestSequence.current) {
        setContentLoading(false);
      }
    }
  }

  async function loadTranslationState(
    relativePath: string,
    requestId: number,
    isActive: () => boolean = () => true
  ) {
    setTranslationChecking(true);
    setTranslationError(null);

    try {
      const nextState = await api.getTranslationState(ability.id, relativePath);
      if (!isActive() || requestId !== readRequestSequence.current) {
        return;
      }
      setTranslationState(nextState);
      setTranslationText(nextState.cached_translation);
    } catch (reason) {
      if (!isActive() || requestId !== readRequestSequence.current) {
        return;
      }
      setTranslationState(null);
      setTranslationText(null);
      setTranslationError(errorMessage(reason));
    } finally {
      if (isActive() && requestId === readRequestSequence.current) {
        setTranslationChecking(false);
      }
    }
  }

  async function handleTranslate() {
    if (!selectedPath || !visibleContent || translationInFlight.current) {
      return;
    }

    const relativePath = selectedPath;
    const requestId = readRequestSequence.current;
    translationInFlight.current = true;
    setTranslationLoading(true);
    setTranslationError(null);

    try {
      const result = await api.translateSkillFile(ability.id, relativePath);
      if (requestId !== readRequestSequence.current || result.relative_path !== selectedPath) {
        return;
      }
      setTranslationText(result.translation);
      setTranslationState({
        skill_id: result.skill_id,
        relative_path: result.relative_path,
        content_hash: result.content_hash,
        show_translate_action: false,
        cached_translation: result.cached ? result.translation : null
      });
    } catch (reason) {
      if (requestId !== readRequestSequence.current) {
        return;
      }
      setTranslationError(errorMessage(reason));
    } finally {
      if (requestId === readRequestSequence.current) {
        setTranslationLoading(false);
        translationInFlight.current = false;
      }
    }
  }

  function resetTranslationState() {
    setTranslationState(null);
    setTranslationText(null);
    setTranslationChecking(false);
    setTranslationLoading(false);
    setTranslationError(null);
    translationInFlight.current = false;
  }

  return (
    <div className="detail-backdrop">
      <section
        aria-label={`Skill 详情 · ${name}`}
        aria-modal="true"
        className="window detail-modal"
        role="dialog"
      >
        <div className="window-bar">
          <span className="dot" aria-hidden="true" />
          <span className="dot" aria-hidden="true" />
          <span className="dot" aria-hidden="true" />
          <span className="bar-title">Skill 详情 · {name}</span>
          <button className="bar-close" onClick={onClose} type="button">
            关闭
          </button>
        </div>

        <div className="modal-body">
          <header className="skill-header">
            <div className="skill-title">
              <h2>{detailInfo?.title ?? name}</h2>
              <p>{summary}</p>
            </div>
            <div className="header-meta">
              <span className="chip accent">{usage}</span>
              <span className="chip">{ability.kind}</span>
              {tags.slice(0, 2).map((tag) => (
                <span className="chip" key={tag}>
                  {tag}
                </span>
              ))}
            </div>
          </header>

          <div className="content-layout">
            <aside className="file-pane">
              <div className="pane-title">
                <strong>文件夹</strong>
                <span>只读</span>
              </div>

              <div className="tree" aria-label="Skill 文件列表">
                {loading ? <div className="file-state">正在读取文件</div> : null}
                {!loading && fileList?.files.length === 0 ? (
                  <div className="file-state">没有可预览文件</div>
                ) : null}
                {fileList?.files.map((entry) => (
                  <button
                    aria-label={entry.relative_path}
                    className={
                      entry.relative_path === selectedPath ? "file selected" : "file"
                    }
                    key={entry.relative_path}
                    onClick={() => handleSelectFile(entry)}
                    type="button"
                  >
                    <span aria-hidden="true">
                      {entry.relative_path === selectedPath ? "▣" : "▢"}
                    </span>
                    <span>{entry.relative_path}</span>
                    <span className="type">{entry.extension?.toUpperCase() ?? "TXT"}</span>
                  </button>
                ))}
              </div>

              <div className="folder-note">
                只列出 Skill 目录内的可阅读文本文件。依赖、构建、二进制和过大文件会被跳过。
              </div>
              {fileList?.warnings.length ? (
                <div className="folder-warning">
                  {fileList.warnings.slice(0, 3).map((warning) => (
                    <p key={warning}>{warning}</p>
                  ))}
                </div>
              ) : null}
            </aside>

            <section className="reader">
              <div className="reader-toolbar">
                <div className="reader-name">
                  <b>{selectedPath ?? "未选择文件"}</b>
                  <span>{rootPath}</span>
                </div>
                <div className="toolbar-actions">
                  {showTranslationAction ? (
                    <button
                      className="action primary"
                      disabled={translationLoading || translationChecking}
                      onClick={handleTranslate}
                      type="button"
                    >
                      {translationLoading ? "翻译中" : "翻译"}
                    </button>
                  ) : null}
                  <button className="action" onClick={onClose} type="button">
                    关闭
                  </button>
                </div>
              </div>

              <div className="reader-content">
                <article className="article">
                  {error ? <div className="notice error">{error}</div> : null}
                  {contentError ? <div className="notice error">{contentError}</div> : null}
                  {translationError ? (
                    <div className="notice error">{translationError}</div>
                  ) : null}
                  {contentLoading ? <div className="list-state">正在读取内容</div> : null}
                  {!contentLoading && !visibleContent && !error && !contentError ? (
                    <div className="list-state">请选择左侧文件</div>
                  ) : null}
                  {displayedContent !== null ? (
                    <pre className="reader-pre">{displayedContent}</pre>
                  ) : null}
                </article>
                <aside className="side-meta">
                  <div className="meta-block">
                    <small>Skill</small>
                    <b>{ability.id}</b>
                    <span>{rootPath}</span>
                  </div>
                  <div className="meta-block">
                    <small>当前文件</small>
                    <b>{selectedEntry?.relative_path ?? selectedPath ?? "未选择"}</b>
                    <span>{formatBytes(selectedEntry?.size_bytes ?? visibleContent?.size_bytes)}</span>
                  </div>
                  <div className="meta-block">
                    <small>语言判断</small>
                    <b>
                      {visibleTranslation
                        ? "中文译文"
                        : contentLanguage === "Chinese"
                          ? "中文内容"
                          : "非中文内容"}
                    </b>
                    <span>{languageNote}</span>
                  </div>
                </aside>
              </div>
            </section>
          </div>
        </div>
      </section>
    </div>
  );
}

function formatBytes(size: number | null | undefined): string {
  if (size === null || size === undefined) {
    return "未知大小";
  }
  if (size < 1024) {
    return `${size} B`;
  }

  return `${(size / 1024).toFixed(1)} KiB`;
}

function errorMessage(reason: unknown): string {
  if (
    reason !== null &&
    typeof reason === "object" &&
    "message" in reason &&
    typeof reason.message === "string"
  ) {
    return reason.message;
  }

  return reason instanceof Error ? reason.message : String(reason);
}
