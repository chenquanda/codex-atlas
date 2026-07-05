import { describe, expect, it } from "vitest";
import { filterAbilities, sortAbilities } from "./abilityFilters";
import { ability } from "../test/fixtures";

describe("abilityFilters", () => {
  it("filters abilities by query, kind, favorite, hidden and tag", () => {
    const abilities = [
      ability({
        id: "skill:brainstorming",
        name: "brainstorming",
        kind: "Skill",
        path: "C:/Users/Administrator/.codex/skills/brainstorming/SKILL.md",
        summary: "Clarify product work before implementation.",
        raw_tags: ["planning"],
        user: {
          alias: null,
          tags: ["需求"],
          tags_overridden: true,
          note: "适合产品设计前的目标澄清",
          favorite: true,
          hidden: false,
          custom_template: null
        }
      }),
      ability({
        id: "plugin:figma-code-connect",
        name: "figma-code-connect",
        kind: "Plugin",
        path: "C:/Users/Administrator/.codex/plugins/cache/figma/code-connect/SKILL.md",
        summary: "Map Figma components to code.",
        raw_tags: ["design"],
        user: {
          alias: null,
          tags: ["design"],
          tags_overridden: true,
          note: "暂时隐藏",
          favorite: false,
          hidden: true,
          custom_template: null
        }
      }),
      ability({
        id: "tool:diagnostics",
        name: "diagnostics",
        kind: "Tool",
        summary: "Inspect runtime state.",
        raw_tags: ["debug"],
        user: {
          alias: null,
          tags: [],
          tags_overridden: false,
          note: null,
          favorite: true,
          hidden: false,
          custom_template: null
        }
      })
    ];

    const visibleSkills = filterAbilities(abilities, {
      query: "产品",
      kind: "Skill",
      favoriteOnly: true,
      hiddenOnly: false,
      tag: "需求",
      sortBy: "usage"
    });
    expect(visibleSkills.map((item) => item.id)).toEqual(["skill:brainstorming"]);

    const hiddenDesign = filterAbilities(abilities, {
      query: "figma",
      kind: "All",
      favoriteOnly: false,
      hiddenOnly: true,
      tag: "design",
      sortBy: "usage"
    });
    expect(hiddenDesign.map((item) => item.id)).toEqual(["plugin:figma-code-connect"]);
  });

  it("sorts usage counts without treating unknown as zero", () => {
    const sorted = sortAbilities(
      [
        ability({ id: "skill:unknown", name: "unknown", stats: { usage_count: null, last_used_at: null } }),
        ability({ id: "skill:zero", name: "zero", stats: { usage_count: 0, last_used_at: null } }),
        ability({ id: "skill:popular", name: "popular", stats: { usage_count: 9, last_used_at: null } })
      ],
      "usage"
    );

    expect(sorted.map((item) => item.id)).toEqual([
      "skill:popular",
      "skill:zero",
      "skill:unknown"
    ]);
  });

  it("searches only effective tags after user tag override", () => {
    const abilities = [
      ability({
        id: "skill:overridden-tags",
        name: "overridden-tags",
        raw_tags: ["raw-debug"],
        ai: {
          summary_zh: null,
          tags: ["ai-design"],
          call_template: "$overridden-tags",
          scenarios: []
        },
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
    ];

    expect(
      filterAbilities(abilities, {
        query: "ai-design",
        kind: "All",
        favoriteOnly: false,
        hiddenOnly: false,
        tag: "",
        sortBy: "usage"
      })
    ).toEqual([]);

    expect(
      filterAbilities(abilities, {
        query: "用户标签",
        kind: "All",
        favoriteOnly: false,
        hiddenOnly: false,
        tag: "",
        sortBy: "usage"
      }).map((item) => item.id)
    ).toEqual(["skill:overridden-tags"]);
  });
});
