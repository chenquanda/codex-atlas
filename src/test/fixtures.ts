import type { Ability } from "../api/atlasApi";

export function ability(overrides: Partial<Ability> = {}): Ability {
  const base: Ability = {
    id: "skill:brainstorming",
    name: "brainstorming",
    kind: "Skill",
    path: "C:/Users/Administrator/.codex/skills/brainstorming/SKILL.md",
    summary: "Clarify goals before implementation.",
    raw_tags: ["流程"],
    ai: {
      summary_zh: "实现前澄清目标、约束和验收标准。",
      tags: ["规划"],
      call_template: "$brainstorming",
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
      usage_count: 12,
      last_used_at: null
    }
  };

  return {
    ...base,
    ...overrides,
    ai: {
      ...base.ai,
      ...overrides.ai
    },
    user: {
      ...base.user,
      ...overrides.user
    },
    stats: {
      ...base.stats,
      ...overrides.stats
    }
  };
}
