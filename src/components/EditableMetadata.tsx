import { useEffect, useState } from "react";
import type { Ability, UserDataPatch } from "../api/atlasApi";

interface EditableMetadataProps {
  ability: Ability;
  onSave: (patch: UserDataPatch) => Promise<void>;
}

export function EditableMetadata({ ability, onSave }: EditableMetadataProps) {
  const [note, setNote] = useState(ability.user.note ?? "");
  const [tags, setTags] = useState(ability.user.tags.join(", "));
  const [tagsDirty, setTagsDirty] = useState(false);
  const [template, setTemplate] = useState(ability.user.custom_template ?? "");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setNote(ability.user.note ?? "");
    setTags(ability.user.tags.join(", "));
    setTagsDirty(false);
    setTemplate(ability.user.custom_template ?? "");
  }, [ability.id, ability.user.custom_template, ability.user.note, ability.user.tags]);

  async function handleSave() {
    setSaving(true);
    try {
      const patch: UserDataPatch = {
        note: normalizeText(note),
        custom_template: normalizeText(template)
      };

      if (tagsDirty) {
        patch.tags = splitTags(tags);
        patch.tags_overridden = true;
      }

      await onSave(patch);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="metadata-editor">
      <label>
        <span>备注</span>
        <textarea
          aria-label="备注"
          rows={3}
          value={note}
          onChange={(event) => setNote(event.currentTarget.value)}
        />
      </label>
      <label>
        <span>标签</span>
        <input
          aria-label="标签编辑"
          value={tags}
          onChange={(event) => {
            setTags(event.currentTarget.value);
            setTagsDirty(true);
          }}
          placeholder="用逗号分隔"
        />
      </label>
      <label>
        <span>自定义模板</span>
        <input
          aria-label="自定义模板"
          value={template}
          onChange={(event) => setTemplate(event.currentTarget.value)}
          placeholder="$skill-name"
        />
      </label>
      <button className="action" disabled={saving} onClick={handleSave} type="button">
        {saving ? "保存中" : "保存元数据"}
      </button>
    </div>
  );
}

function normalizeText(value: string): string | null {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function splitTags(value: string): string[] {
  return value
    .split(/[,\n，]/)
    .map((tag) => tag.trim())
    .filter(Boolean);
}
