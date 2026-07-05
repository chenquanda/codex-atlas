import type { Ability } from "../api/atlasApi";

interface MetricCardsProps {
  abilities: Ability[];
}

export function MetricCards({ abilities }: MetricCardsProps) {
  const skillCount = abilities.filter((ability) => ability.kind === "Skill").length;
  const usedCount = abilities.filter((ability) => (ability.stats.usage_count ?? 0) > 0).length;
  const favoriteCount = abilities.filter((ability) => ability.user.favorite).length;

  const metrics = [
    { label: "Skills", value: skillCount.toString() },
    { label: "已使用", value: usedCount.toString() },
    { label: "收藏", value: favoriteCount.toString() }
  ];

  return (
    <div className="stats-line" aria-label="能力概览">
      {metrics.map((metric) => (
        <div className="meter" key={metric.label}>
          <b>{metric.value}</b>
          <span>{metric.label}</span>
        </div>
      ))}
    </div>
  );
}
