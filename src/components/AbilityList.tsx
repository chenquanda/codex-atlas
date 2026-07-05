import type { Ability } from "../api/atlasApi";
import {
  formatUsageLabel,
  getAbilityName,
  getAbilitySummary,
  getAbilityTags,
  getLanguageLabel
} from "../lib/abilityFormatting";

interface AbilityListProps {
  abilities: Ability[];
  selectedId: string | null;
  loading: boolean;
  onSelect: (ability: Ability) => void;
}

export function AbilityList({ abilities, selectedId, loading, onSelect }: AbilityListProps) {
  return (
    <div className="ability-list" aria-label="能力列表">
      <div className="list-head">
        <span>能力</span>
        <span>使用</span>
        <span>语言</span>
      </div>
      {loading ? <div className="list-state">正在读取能力索引</div> : null}
      {!loading && abilities.length === 0 ? (
        <div className="list-state">没有符合筛选条件的能力</div>
      ) : null}
      {abilities.map((ability) => {
        const name = getAbilityName(ability);
        const summary = getAbilitySummary(ability);
        const usage = formatUsageLabel(ability.stats.usage_count);
        const language = getLanguageLabel(ability);
        const tags = getAbilityTags(ability);

        return (
          <button
            aria-label={`${name} ${summary} ${usage} ${language.label}`}
            className={ability.id === selectedId ? "ability selected" : "ability"}
            key={ability.id}
            onClick={() => onSelect(ability)}
            type="button"
          >
            <span className="ability-main">
              <span className="ability-name">
                <span className={ability.user.favorite ? "star active" : "star"} aria-hidden="true">
                  {ability.user.favorite ? "★" : ""}
                </span>
                <b>{name}</b>
              </span>
              <span className="summary">{summary}</span>
              <span className="tag-row">
                {tags.slice(0, 4).map((tag) => (
                  <span className="tag" key={tag}>
                    {tag}
                  </span>
                ))}
              </span>
            </span>
            <span className="usage">{usage}</span>
            <span className={language.cached ? "lang cached" : "lang"}>{language.label}</span>
          </button>
        );
      })}
    </div>
  );
}
