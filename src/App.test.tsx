// @vitest-environment jsdom
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { AtlasApi, UserDataPatch } from "./api/atlasApi";
import { ability } from "./test/fixtures";

afterEach(() => {
  cleanup();
  window.history.pushState({}, "", "/");
  delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

interface TestSkillFileEntry {
  relative_path: string;
  size_bytes: number;
  extension: string | null;
}

interface TestSkillFileList {
  skill_id: string;
  root_path: string;
  files: TestSkillFileEntry[];
  warnings: string[];
}

interface TestSkillFileContent {
  skill_id: string;
  relative_path: string;
  content: string;
  size_bytes: number;
  language: "Chinese" | "Other";
}

interface TestSkillDetailInfo {
  skill_id: string;
  title: string;
  root_path: string;
}

interface DetailApiPart {
  openSkillDetailWindow: (id: string) => Promise<TestSkillDetailInfo>;
  listSkillFiles: (id: string) => Promise<TestSkillFileList>;
  readSkillFile: (id: string, relativePath: string) => Promise<TestSkillFileContent>;
}

interface CreateApiOptions {
  files?: TestSkillFileEntry[];
  contents?: Record<string, string>;
  readSkillFile?: (id: string, relativePath: string) => Promise<TestSkillFileContent>;
}

function skillContent(
  id: string,
  relativePath: string,
  content: string
): TestSkillFileContent {
  return {
    skill_id: id,
    relative_path: relativePath,
    content,
    size_bytes: content.length,
    language: /[\u4e00-\u9fff]/.test(content) ? "Chinese" : "Other"
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });

  return { promise, resolve };
}

function createApi(abilities = [ability()], options: CreateApiOptions = {}): AtlasApi & DetailApiPart {
  let currentAbilities = abilities;
  const files = options.files ?? [
    { relative_path: "SKILL.md", size_bytes: 24, extension: "md" },
    { relative_path: "references/guide.md", size_bytes: 18, extension: "md" }
  ];
  const contents = options.contents ?? {
    "SKILL.md": "# Reader\n\nPrimary skill file.",
    "references/guide.md": "Guide content."
  };

  return {
    listAbilities: vi.fn(async () => currentAbilities),
    updateUserData: vi.fn(async (id: string, patch: UserDataPatch) => {
      currentAbilities = currentAbilities.map((item) =>
        item.id === id
          ? {
              ...item,
              user: {
                ...item.user,
                alias: patch.alias === undefined ? item.user.alias : patch.alias,
                custom_template:
                  patch.custom_template === undefined
                    ? item.user.custom_template
                    : patch.custom_template,
                favorite: patch.favorite ?? item.user.favorite,
                hidden: patch.hidden ?? item.user.hidden,
                note: patch.note === undefined ? item.user.note : patch.note,
                tags: patch.tags ?? item.user.tags,
                tags_overridden: patch.tags ? true : item.user.tags_overridden
              }
            }
          : item
      );

      return currentAbilities.find((item) => item.id === id) ?? abilities[0];
    }),
    refreshScan: vi.fn(async () => ({
      ability_count: currentAbilities.length,
      warning_count: 0,
      warnings: []
    })),
    refreshStats: vi.fn(async () => ({
      ability_count: currentAbilities.length,
      warning_count: 0,
      warnings: [],
      complete: true,
      saved_cache: true
    })),
    copyCallTemplate: vi.fn(async (id: string) => {
      const target = currentAbilities.find((item) => item.id === id);
      return target?.user.custom_template ?? target?.ai.call_template ?? `$${target?.name ?? id}`;
    }),
    openSkillDetailWindow: vi.fn(async (id: string) => {
      const target = currentAbilities.find((item) => item.id === id);
      return {
        skill_id: id,
        title: target?.name ?? id,
        root_path: String(target?.path ?? "")
      };
    }),
    listSkillFiles: vi.fn(async (id: string) => ({
      skill_id: id,
      root_path: "C:/fixtures/skills/reader",
      files,
      warnings: []
    })),
    readSkillFile: vi.fn(async (id: string, relativePath: string) => {
      if (options.readSkillFile) {
        return options.readSkillFile(id, relativePath);
      }
      const content = contents[relativePath] ?? "";
      return skillContent(id, relativePath, content);
    })
  };
}

function markTauriRuntime() {
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {}
  });
}

describe("App", () => {
  it("renders unknown usage as 未统计", async () => {
    const api = createApi([
      ability({
        id: "skill:documents",
        name: "documents",
        stats: { usage_count: null, last_used_at: null }
      })
    ]);

    render(<App api={api} />);

    const row = await screen.findByRole("button", { name: /documents/ });
    expect(within(row).getByText("未统计")).toBeTruthy();
    expect(within(row).queryByText("0 使用")).toBeNull();
  });

  it("copies the user custom template before AI template into the clipboard", async () => {
    const api = createApi([
      ability({
        id: "skill:custom-template",
        name: "custom-template",
        ai: {
          summary_zh: null,
          tags: ["流程"],
          call_template: "$ai-template",
          scenarios: []
        },
        user: {
          alias: null,
          tags: [],
          tags_overridden: false,
          note: null,
          favorite: false,
          hidden: false,
          custom_template: "$user-template"
        }
      })
    ]);
    const writeClipboardText = vi.fn(async () => undefined);
    const user = userEvent.setup();

    render(<App api={api} writeClipboardText={writeClipboardText} />);
    await user.click(await screen.findByRole("button", { name: /custom-template/ }));
    await user.click(screen.getByRole("button", { name: "复制模板" }));

    expect(api.copyCallTemplate).toHaveBeenCalledWith("skill:custom-template");
    expect(writeClipboardText).toHaveBeenCalledWith("$user-template");
    expect(await screen.findByText("已复制 $user-template")).toBeTruthy();
  });

  it("does not report copied when writing the clipboard fails", async () => {
    const api = createApi([
      ability({
        id: "skill:clipboard-failure",
        name: "clipboard-failure",
        user: {
          alias: null,
          tags: [],
          tags_overridden: false,
          note: null,
          favorite: false,
          hidden: false,
          custom_template: "$user-template"
        }
      })
    ]);
    const writeClipboardText = vi.fn(async () => {
      throw new Error("剪贴板不可用");
    });
    const user = userEvent.setup();

    render(<App api={api} writeClipboardText={writeClipboardText} />);
    await user.click(await screen.findByRole("button", { name: /clipboard-failure/ }));
    await user.click(screen.getByRole("button", { name: "复制模板" }));

    expect(writeClipboardText).toHaveBeenCalledWith("$user-template");
    expect(screen.queryByText("已复制 $user-template")).toBeNull();
    expect(screen.queryByText("调用模板已复制")).toBeNull();
    expect(await screen.findByText("剪贴板不可用")).toBeTruthy();
  });

  it("does not submit tag overrides when only saving a note", async () => {
    const api = createApi([
      ability({
        id: "skill:note-only",
        name: "note-only",
        ai: {
          summary_zh: "编辑备注时保留 AI 标签。",
          tags: ["AI标签"],
          call_template: "$note-only",
          scenarios: []
        },
        user: {
          alias: null,
          tags: [],
          tags_overridden: false,
          note: null,
          favorite: false,
          hidden: false,
          custom_template: null
        }
      })
    ]);
    const user = userEvent.setup();

    render(<App api={api} />);
    await user.click(await screen.findByRole("button", { name: /note-only/ }));
    await user.click(screen.getByRole("button", { name: "更多" }));
    await user.type(screen.getByLabelText("备注"), "只改备注");
    await user.click(screen.getByRole("button", { name: "保存元数据" }));

    expect(api.updateUserData).toHaveBeenCalledWith(
      "skill:note-only",
      expect.not.objectContaining({
        tags: expect.anything()
      })
    );
    expect(api.updateUserData).toHaveBeenCalledWith(
      "skill:note-only",
      expect.not.objectContaining({
        tags_overridden: expect.anything()
      })
    );
    expect((await screen.findAllByText("AI标签")).length).toBeGreaterThan(0);
  });

  it("submits an empty tag override when the user explicitly clears tags", async () => {
    const api = createApi([
      ability({
        id: "skill:clear-tags",
        name: "clear-tags",
        user: {
          alias: null,
          tags: ["用户标签"],
          tags_overridden: true,
          note: null,
          favorite: false,
          hidden: false,
          custom_template: null
        }
      })
    ]);
    const user = userEvent.setup();

    render(<App api={api} />);
    await user.click(await screen.findByRole("button", { name: /clear-tags/ }));
    await user.click(screen.getByRole("button", { name: "更多" }));
    await user.clear(screen.getByLabelText("标签编辑"));
    await user.click(screen.getByRole("button", { name: "保存元数据" }));

    expect(api.updateUserData).toHaveBeenCalledWith(
      "skill:clear-tags",
      expect.objectContaining({
        tags: [],
        tags_overridden: true
      })
    );
  });

  it("does not show scan success if reloading abilities fails", async () => {
    const api = createApi([ability({ id: "skill:scan", name: "scan" })]);
    vi.mocked(api.listAbilities)
      .mockResolvedValueOnce([ability({ id: "skill:scan", name: "scan" })])
      .mockRejectedValueOnce(new Error("列表刷新失败"));
    const user = userEvent.setup();

    render(<App api={api} />);
    await screen.findByRole("button", { name: /scan/ });
    await user.click(screen.getByRole("button", { name: "扫描" }));

    expect(await screen.findByText("列表刷新失败")).toBeTruthy();
    expect(screen.queryByText("扫描已刷新")).toBeNull();
  });

  it("clears stale copy success when a later copy fails", async () => {
    const api = createApi([
      ability({
        id: "skill:copy-again",
        name: "copy-again",
        user: {
          alias: null,
          tags: [],
          tags_overridden: false,
          note: null,
          favorite: false,
          hidden: false,
          custom_template: "$copy-again"
        }
      })
    ]);
    const writeClipboardText = vi
      .fn()
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error("剪贴板写入失败"));
    const user = userEvent.setup();

    render(<App api={api} writeClipboardText={writeClipboardText} />);
    await user.click(await screen.findByRole("button", { name: /copy-again/ }));
    await user.click(screen.getByRole("button", { name: "复制模板" }));
    expect(await screen.findByText("已复制 $copy-again")).toBeTruthy();
    expect(await screen.findByText("调用模板已复制")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "复制模板" }));

    expect(await screen.findByText("剪贴板写入失败")).toBeTruthy();
    expect(screen.queryByText("已复制 $copy-again")).toBeNull();
    expect(screen.queryByText("调用模板已复制")).toBeNull();
  });

  it("expands low-frequency actions behind the 更多 button", async () => {
    const api = createApi([
      ability({
        id: "skill:metadata",
        name: "metadata",
        user: {
          alias: null,
          tags: ["流程"],
          tags_overridden: true,
          note: "现有备注",
          favorite: false,
          hidden: false,
          custom_template: null
        }
      })
    ]);
    const user = userEvent.setup();

    render(<App api={api} />);
    await user.click(await screen.findByRole("button", { name: /metadata/ }));

    expect(screen.queryByRole("button", { name: "隐藏能力" })).toBeNull();
    expect(screen.queryByLabelText("备注")).toBeNull();

    await user.click(screen.getByRole("button", { name: "更多" }));

    expect(screen.getByRole("button", { name: "隐藏能力" })).toBeTruthy();
    expect(screen.getByLabelText("备注")).toBeTruthy();
  });

  it("opens detail reader for a skill and lists files", async () => {
    const api = createApi([ability({ id: "skill:details", name: "details" })]);
    const user = userEvent.setup();

    render(<App api={api} />);
    await screen.findByRole("button", { name: /details/ });

    await user.click(screen.getByRole("button", { name: "打开详情" }));

    expect(api.openSkillDetailWindow).toHaveBeenCalledWith("skill:details");
    expect(api.listSkillFiles).toHaveBeenCalledWith("skill:details");
    expect(await screen.findByRole("dialog", { name: /Skill 详情/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: "SKILL.md" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "references/guide.md" })).toBeTruthy();
  });

  it("does not open an overlay in the main window when running inside Tauri", async () => {
    markTauriRuntime();
    const api = createApi([ability({ id: "skill:details", name: "details" })]);
    const user = userEvent.setup();

    render(<App api={api} />);
    await screen.findByRole("button", { name: /details/ });

    await user.click(screen.getByRole("button", { name: "打开详情" }));

    await waitFor(() =>
      expect(api.openSkillDetailWindow).toHaveBeenCalledWith("skill:details")
    );
    expect(screen.queryByRole("dialog", { name: /Skill 详情/ })).toBeNull();
    expect(api.listSkillFiles).not.toHaveBeenCalled();
  });

  it("shows the selected file content on the right", async () => {
    const api = createApi([ability({ id: "skill:details", name: "details" })], {
      contents: {
        "SKILL.md": "# Reader\n\nPrimary skill file.",
        "references/guide.md": "Selected guide content."
      }
    });
    const user = userEvent.setup();

    render(<App api={api} />);
    await screen.findByRole("button", { name: /details/ });
    await user.click(screen.getByRole("button", { name: "打开详情" }));

    expect(await screen.findByText(/Primary skill file/)).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "references/guide.md" }));

    expect(await screen.findByText("Selected guide content.")).toBeTruthy();
    expect(api.readSkillFile).toHaveBeenLastCalledWith("skill:details", "references/guide.md");
  });

  it("clears stale content and keeps the latest selected file when reads finish out of order", async () => {
    const slowRead = deferred<TestSkillFileContent>();
    const api = createApi([ability({ id: "skill:details", name: "details" })], {
      files: [
        { relative_path: "SKILL.md", size_bytes: 18, extension: "md" },
        { relative_path: "slow.md", size_bytes: 21, extension: "md" },
        { relative_path: "fast.md", size_bytes: 20, extension: "md" }
      ],
      readSkillFile: async (id, relativePath) => {
        if (relativePath === "slow.md") {
          return slowRead.promise;
        }

        return skillContent(
          id,
          relativePath,
          relativePath === "fast.md" ? "Fast selected content." : "Initial skill content."
        );
      }
    });
    const user = userEvent.setup();

    render(<App api={api} />);
    await screen.findByRole("button", { name: /details/ });
    await user.click(screen.getByRole("button", { name: "打开详情" }));
    expect(await screen.findByText("Initial skill content.")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "slow.md" }));

    expect(screen.queryByText("Initial skill content.")).toBeNull();
    expect(screen.getByText("正在读取内容")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "fast.md" }));
    expect(await screen.findByText("Fast selected content.")).toBeTruthy();

    slowRead.resolve(skillContent("skill:details", "slow.md", "Slow stale content."));

    await waitFor(() => {
      expect(screen.getByText("Fast selected content.")).toBeTruthy();
      expect(screen.queryByText("Slow stale content.")).toBeNull();
    });
  });

  it("does not show translation controls for Chinese content", async () => {
    const api = createApi([ability({ id: "skill:zh", name: "zh" })], {
      files: [{ relative_path: "README.zh.md", size_bytes: 24, extension: "md" }],
      contents: {
        "README.zh.md": "这是中文 Skill 文档，不需要翻译按钮。"
      }
    });
    const user = userEvent.setup();

    render(<App api={api} />);
    await screen.findByRole("button", { name: /zh/ });
    await user.click(screen.getByRole("button", { name: "打开详情" }));

    expect(await screen.findByText("这是中文 Skill 文档，不需要翻译按钮。")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /翻译/ })).toBeNull();
  });

  it("opens the detail reader from a skillDetail URL parameter", async () => {
    window.history.pushState({}, "", "/?skillDetail=skill%3Adetails");
    const api = createApi([ability({ id: "skill:details", name: "details" })]);

    render(<App api={api} />);

    expect(await screen.findByRole("dialog", { name: /Skill 详情 · details/ })).toBeTruthy();
    expect(api.listSkillFiles).toHaveBeenCalledWith("skill:details");
    expect(await screen.findByText(/Primary skill file/)).toBeTruthy();
  });

  it("consumes the skillDetail URL parameter so closing the detail reader does not reopen it", async () => {
    window.history.pushState({}, "", "/?skillDetail=skill%3Adetails&keep=1#x");
    const api = createApi([ability({ id: "skill:details", name: "details" })]);
    const user = userEvent.setup();

    render(<App api={api} />);

    const dialog = await screen.findByRole("dialog", { name: /Skill 详情 · details/ });
    expect(window.location.search).toBe("?keep=1");
    expect(window.location.hash).toBe("#x");
    expect(new URLSearchParams(window.location.search).has("skillDetail")).toBe(false);

    await user.click(within(dialog).getAllByRole("button", { name: "关闭" })[0]);

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /Skill 详情/ })).toBeNull();
    });
    expect(api.listSkillFiles).toHaveBeenCalledTimes(1);
    expect(window.location.search).toBe("?keep=1");
    expect(window.location.hash).toBe("#x");
  });
});
