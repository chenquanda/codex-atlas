import type { Ability, AbilityKind } from "../api/atlasApi";
import { getAbilityName, getAbilitySummary, getAbilityTags } from "./abilityFormatting";

export type KindFilter = AbilityKind | "All";
export type AbilitySort = "usage" | "name" | "kind";

export interface AbilityFilterState {
  query: string;
  kind: KindFilter;
  favoriteOnly: boolean;
  hiddenOnly: boolean;
  tag: string;
  sortBy: AbilitySort;
}

const kindOrder: Record<AbilityKind, number> = {
  Skill: 0,
  Plugin: 1,
  Tool: 2,
  App: 3
};

export function filterAbilities(
  abilities: Ability[],
  filters: AbilityFilterState
): Ability[] {
  return sortAbilities(
    abilities.filter((ability) => {
      if (filters.kind !== "All" && ability.kind !== filters.kind) {
        return false;
      }

      if (filters.favoriteOnly && !ability.user.favorite) {
        return false;
      }

      if (filters.hiddenOnly !== ability.user.hidden) {
        return false;
      }

      if (filters.tag && !getAbilityTags(ability).includes(filters.tag)) {
        return false;
      }

      return matchesQuery(ability, filters.query);
    }),
    filters.sortBy
  );
}

export function sortAbilities(abilities: Ability[], sortBy: AbilitySort): Ability[] {
  return [...abilities].sort((left, right) => {
    if (sortBy === "usage") {
      const usage = compareUsage(left, right);
      if (usage !== 0) {
        return usage;
      }
    }

    if (sortBy === "kind") {
      const kind = kindOrder[left.kind] - kindOrder[right.kind];
      if (kind !== 0) {
        return kind;
      }
    }

    return getAbilityName(left).localeCompare(getAbilityName(right), "zh-Hans-CN");
  });
}

function compareUsage(left: Ability, right: Ability): number {
  const leftUsage = left.stats.usage_count;
  const rightUsage = right.stats.usage_count;
  const leftUnknown = leftUsage === null || leftUsage === undefined;
  const rightUnknown = rightUsage === null || rightUsage === undefined;

  if (leftUnknown && rightUnknown) {
    return 0;
  }

  if (leftUnknown) {
    return 1;
  }

  if (rightUnknown) {
    return -1;
  }

  return rightUsage - leftUsage;
}

function matchesQuery(ability: Ability, query: string): boolean {
  const normalized = normalize(query);
  if (!normalized) {
    return true;
  }

  const fields = [
    ability.id,
    ability.name,
    ability.user.alias,
    ability.summary,
    ability.ai.summary_zh,
    ability.user.note,
    ability.path,
    ...getAbilityTags(ability),
    getAbilityName(ability),
    getAbilitySummary(ability)
  ];

  return fields.some((field) => normalize(field).includes(normalized));
}

function normalize(value: string | null | undefined): string {
  return (value ?? "").trim().toLowerCase();
}
