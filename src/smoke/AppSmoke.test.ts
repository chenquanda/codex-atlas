import { expect, type Locator, type Page, test } from "@playwright/test";

const smokeAbilities = [
  ability({
    id: "skill:atlas-smoke",
    name: "atlas-smoke",
    path: "C:/Users/Administrator/.codex/skills/atlas-smoke/SKILL.md",
    summary: "Smoke data for the browser shell and detail reader.",
    raw_tags: ["smoke", "reader"],
    ai: {
      summary_zh: "用于浏览器 smoke 的 Skill 数据。",
      tags: ["smoke", "详情"],
      call_template: "$atlas-smoke",
      scenarios: []
    },
    stats: {
      usage_count: 42,
      last_used_at: null
    }
  }),
  ability({
    id: "plugin:sample-plugin",
    name: "sample-plugin",
    kind: "Plugin",
    path: "C:/Users/Administrator/.codex/plugins/sample/plugin.json",
    summary: "Plugin row used to exercise toolbar filtering.",
    raw_tags: ["plugin"],
    stats: {
      usage_count: null,
      last_used_at: null
    }
  })
];

const smokeFiles = [
  { relative_path: "SKILL.md", size_bytes: 58, extension: "md" },
  { relative_path: "references/usage.md", size_bytes: 46, extension: "md" }
];

const smokeContents: Record<string, string> = {
  "SKILL.md": "# Atlas Smoke\n\nPrimary skill file for GUI smoke.",
  "references/usage.md": "Selected reference content for the smoke reader."
};

test.beforeEach(async ({ page }) => {
  await installMockedTauriApi(page);
});

test("renders the second shell and keeps main controls hittable", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("main", { name: "Codex Atlas" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "能力索引" })).toBeVisible();
  await expect(page.getByRole("button", { name: /atlas-smoke/ })).toBeVisible();
  await expect(page.getByRole("button", { name: "扫描" })).toBeVisible();
  await expect(page.getByRole("button", { name: "统计" })).toBeVisible();
  await expect(page.getByLabel("搜索")).toBeVisible();
  await expect(page.getByRole("button", { name: "打开详情" })).toBeEnabled();

  await expectCenterIsHittable(page.getByRole("button", { name: "扫描" }));
  await expectCenterIsHittable(page.getByRole("button", { name: "Skills" }));
  await expectCenterIsHittable(page.getByRole("button", { name: "打开详情" }));

  const hasNoHorizontalOverflow = await page.evaluate(
    () => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1
  );
  expect(hasNoHorizontalOverflow).toBe(true);
});

test("opens skill detail, selects a file, and shows mocked content", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "打开详情" }).click();

  const dialog = page.getByRole("dialog", { name: /Skill 详情 · atlas-smoke/ });
  await expect(dialog).toBeVisible();
  await expect(page.getByText("Primary skill file for GUI smoke.")).toBeVisible();

  await page.getByRole("button", { name: "references/usage.md" }).click();

  await expect(page.getByText("Selected reference content for the smoke reader.")).toBeVisible();
  await expect(page.getByRole("button", { name: "翻译" })).toBeVisible();

  await page.getByRole("button", { name: "翻译" }).click();

  await expect(page.getByText("GUI smoke 手动翻译结果。")).toBeVisible();
  await expectCenterIsHittable(page.getByRole("button", { name: "关闭" }).last());
});

async function installMockedTauriApi(page: Page) {
  await page.addInitScript(
    ({ abilities, files, contents }) => {
      type InvokeArgs = Record<string, unknown> | undefined;

      const win = window as typeof window & {
        __CODEX_ATLAS_BROWSER_SMOKE__?: boolean;
        __TAURI_INTERNALS__?: {
          invoke: (command: string, args?: InvokeArgs) => Promise<unknown>;
          transformCallback: (callback?: (payload: unknown) => void) => number;
          unregisterCallback: (id: number) => void;
          callbacks: Record<number, (payload: unknown) => void>;
        };
      };
      const callbacks: Record<number, (payload: unknown) => void> = {};
      let nextCallbackId = 1;

      // 浏览器 smoke 只 mock IPC 数据，不启动真实 Tauri runtime，也不调用真实 Codex 命令。
      win.__CODEX_ATLAS_BROWSER_SMOKE__ = true;
      win.__TAURI_INTERNALS__ = {
        callbacks,
        transformCallback(callback) {
          const id = nextCallbackId;
          nextCallbackId += 1;
          callbacks[id] = callback ?? (() => undefined);
          return id;
        },
        unregisterCallback(id) {
          delete callbacks[id];
        },
        async invoke(command, args) {
          const input = args ?? {};
          const id = String(input.id ?? "skill:atlas-smoke");
          const relativePath = String(input.relative_path ?? "SKILL.md");

          switch (command) {
            case "list_abilities":
              return abilities;
            case "refresh_scan":
              return { ability_count: abilities.length, warning_count: 0, warnings: [] };
            case "refresh_stats":
              return {
                ability_count: abilities.length,
                warning_count: 0,
                warnings: [],
                complete: true,
                saved_cache: true
              };
            case "copy_call_template":
              return "$atlas-smoke";
            case "open_skill_detail_window":
              return {
                skill_id: id,
                title: "atlas-smoke",
                root_path: "C:/Users/Administrator/.codex/skills/atlas-smoke"
              };
            case "list_skill_files":
              return {
                skill_id: id,
                root_path: "C:/Users/Administrator/.codex/skills/atlas-smoke",
                files,
                warnings: []
              };
            case "read_skill_file": {
              const content = contents[relativePath] ?? "";
              return {
                skill_id: id,
                relative_path: relativePath,
                content,
                size_bytes: content.length,
                language: /[\u4e00-\u9fff]/.test(content) ? "Chinese" : "Other"
              };
            }
            case "get_translation_state":
              return {
                skill_id: id,
                relative_path: relativePath,
                content_hash: `smoke-${relativePath.length}`,
                show_translate_action: true,
                cached_translation: null
              };
            case "translate_skill_file":
              return {
                skill_id: id,
                relative_path: relativePath,
                content_hash: `smoke-${relativePath.length}`,
                translation: "GUI smoke 手动翻译结果。",
                cached: false
              };
            default:
              throw new Error(`未 mock 的 Tauri command: ${command}`);
          }
        }
      };
    },
    {
      abilities: smokeAbilities,
      files: smokeFiles,
      contents: smokeContents
    }
  );
}

async function expectCenterIsHittable(locator: Locator) {
  await expect(locator).toBeVisible();
  const hittable = await locator.evaluate((element) => {
    const rect = element.getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    const hit = document.elementFromPoint(x, y);

    return hit === element || element.contains(hit);
  });

  expect(hittable).toBe(true);
}

function ability(overrides: Record<string, unknown> = {}) {
  const base = {
    id: "skill:atlas-smoke",
    name: "atlas-smoke",
    kind: "Skill",
    path: "C:/Users/Administrator/.codex/skills/atlas-smoke/SKILL.md",
    summary: "Smoke data.",
    raw_tags: ["smoke"],
    ai: {
      summary_zh: null,
      tags: [],
      call_template: "$atlas-smoke",
      scenarios: []
    },
    user: {
      alias: null,
      tags: [],
      tags_overridden: false,
      note: null,
      favorite: true,
      hidden: false,
      custom_template: null
    },
    stats: {
      usage_count: null,
      last_used_at: null
    }
  };

  return {
    ...base,
    ...overrides,
    ai: {
      ...base.ai,
      ...((overrides.ai as Record<string, unknown> | undefined) ?? {})
    },
    user: {
      ...base.user,
      ...((overrides.user as Record<string, unknown> | undefined) ?? {})
    },
    stats: {
      ...base.stats,
      ...((overrides.stats as Record<string, unknown> | undefined) ?? {})
    }
  };
}
