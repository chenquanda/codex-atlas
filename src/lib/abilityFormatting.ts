import type { Ability, AbilityKind } from "../api/atlasApi";

export function formatUsageLabel(usageCount: number | null | undefined): string {
  return usageCount === null || usageCount === undefined ? "未统计" : `${usageCount} 使用`;
}

export function getAbilityName(ability: Ability): string {
  return nonEmpty(ability.user.alias) ?? ability.name;
}

export function getAbilitySummary(ability: Ability): string {
  return nonEmpty(ability.ai.summary_zh) ?? ability.summary;
}

export function getAbilityTags(ability: Ability): string[] {
  if (ability.user.tags_overridden || ability.user.tags.length > 0) {
    return ability.user.tags;
  }

  if (ability.ai.tags.length > 0) {
    return ability.ai.tags;
  }

  return ability.raw_tags;
}

export function getKindLabel(kind: AbilityKind): string {
  const labels: Record<AbilityKind, string> = {
    Skill: "Skill",
    Plugin: "Plugin",
    Tool: "Tool",
    App: "App"
  };

  return labels[kind];
}

export function getLanguageLabel(ability: Ability): { label: string; cached: boolean } {
  if (nonEmpty(ability.ai.summary_zh)) {
    return { label: "中文优先", cached: true };
  }

  if (containsChinese(ability.summary) || containsChinese(ability.user.note ?? "")) {
    return { label: "中文", cached: false };
  }

  return { label: "英文", cached: false };
}

export function uniqueAbilityTags(abilities: Ability[]): string[] {
  return Array.from(new Set(abilities.flatMap(getAbilityTags))).sort((left, right) =>
    left.localeCompare(right, "zh-Hans-CN")
  );
}

function containsChinese(value: string): boolean {
  return /[\u4e00-\u9fff]/.test(value);
}

function nonEmpty(value: string | null | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}
