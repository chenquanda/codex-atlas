import type { KindFilter } from "../lib/abilityFilters";

interface AbilityToolbarProps {
  value: KindFilter;
  onChange: (kind: KindFilter) => void;
}

const kindOptions: Array<{ label: string; value: KindFilter }> = [
  { label: "Skills", value: "Skill" },
  { label: "Plugins", value: "Plugin" },
  { label: "Tools", value: "Tool" },
  { label: "Apps", value: "App" },
  { label: "All", value: "All" }
];

export function AbilityToolbar({ value, onChange }: AbilityToolbarProps) {
  return (
    <div className="segmented" aria-label="能力类型">
      {kindOptions.map((option) => (
        <button
          className={option.value === value ? "segment active" : "segment"}
          key={option.value}
          onClick={() => onChange(option.value)}
          type="button"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
