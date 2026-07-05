interface SearchBoxProps {
  value: string;
  onChange: (value: string) => void;
}

export function SearchBox({ value, onChange }: SearchBoxProps) {
  return (
    <label className="search">
      <span className="prompt" aria-hidden="true">
        &gt;
      </span>
      <input
        aria-label="搜索"
        placeholder="搜索 id、名称、摘要、标签、备注或路径"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}
