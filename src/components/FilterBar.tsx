import type { AbilitySort } from "../lib/abilityFilters";

interface FilterBarProps {
  favoriteOnly: boolean;
  hiddenOnly: boolean;
  tag: string;
  tagOptions: string[];
  sortBy: AbilitySort;
  onFavoriteOnlyChange: (value: boolean) => void;
  onHiddenOnlyChange: (value: boolean) => void;
  onTagChange: (value: string) => void;
  onSortChange: (value: AbilitySort) => void;
}

export function FilterBar({
  favoriteOnly,
  hiddenOnly,
  tag,
  tagOptions,
  sortBy,
  onFavoriteOnlyChange,
  onHiddenOnlyChange,
  onTagChange,
  onSortChange
}: FilterBarProps) {
  return (
    <div className="filter-line">
      <div className="filters" aria-label="筛选">
        <button
          className={favoriteOnly ? "small-btn active" : "small-btn"}
          onClick={() => onFavoriteOnlyChange(!favoriteOnly)}
          type="button"
        >
          常用
        </button>
        <button
          className={hiddenOnly ? "small-btn active" : "small-btn"}
          onClick={() => onHiddenOnlyChange(!hiddenOnly)}
          type="button"
        >
          隐藏
        </button>
        <label className="select-label">
          <span>标签</span>
          <select
            aria-label="标签"
            value={tag}
            onChange={(event) => onTagChange(event.currentTarget.value)}
          >
            <option value="">全部标签</option>
            {tagOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
      </div>
      <label className="sort">
        <span>排序</span>
        <select
          aria-label="排序"
          value={sortBy}
          onChange={(event) => onSortChange(event.currentTarget.value as AbilitySort)}
        >
          <option value="usage">使用次数</option>
          <option value="name">名称</option>
          <option value="kind">类型</option>
        </select>
      </label>
    </div>
  );
}
