import type { Ability, UserDataPatch } from "../api/atlasApi";
import {
  formatUsageLabel,
  getAbilityName,
  getAbilitySummary,
  getAbilityTags
} from "../lib/abilityFormatting";
import { EditableMetadata } from "./EditableMetadata";

interface BottomPanelProps {
  ability: Ability | null;
  copyStatus: string | null;
  moreOpen: boolean;
  onCopy: (ability: Ability) => Promise<void>;
  onOpenDetail: (ability: Ability) => void;
  onMoreChange: (open: boolean) => void;
  onUpdateUserData: (ability: Ability, patch: UserDataPatch) => Promise<void>;
}

export function BottomPanel({
  ability,
  copyStatus,
  moreOpen,
  onCopy,
  onOpenDetail,
  onMoreChange,
  onUpdateUserData
}: BottomPanelProps) {
  if (!ability) {
    return (
      <div className="bottom-panel">
        <p className="empty-detail">选择一个能力后可复制调用模板或编辑元数据。</p>
      </div>
    );
  }

  const name = getAbilityName(ability);
  const usage = formatUsageLabel(ability.stats.usage_count);

  return (
    <div className="bottom-panel">
      <div className="detail-grid">
        <div className="detail-copy">
          <h3 className="detail-title">{name}</h3>
          <div className="detail-path">{ability.path ?? ability.id}</div>
          <p className="detail-summary">{getAbilitySummary(ability)}</p>
          <div className="detail-tags">
            <span>{usage}</span>
            {getAbilityTags(ability).map((tag) => (
              <span className="tag" key={tag}>
                {tag}
              </span>
            ))}
          </div>
          {copyStatus ? (
            <div className="copy-status" role="status">
              {copyStatus}
            </div>
          ) : null}
        </div>
        <div className="actions">
          <button
            className="action primary"
            disabled={ability.kind !== "Skill"}
            onClick={() => onOpenDetail(ability)}
            title={ability.kind === "Skill" ? "打开 Skill 文件阅读器" : "仅 Skill 支持详情阅读"}
            type="button"
          >
            打开详情
          </button>
          <button className="action" onClick={() => onCopy(ability)} type="button">
            复制模板
          </button>
          <button
            aria-expanded={moreOpen}
            className="action"
            onClick={() => onMoreChange(!moreOpen)}
            type="button"
          >
            更多
          </button>
        </div>
      </div>

      {moreOpen ? (
        <div className="more-panel">
          <div className="more-actions">
            <button
              className="action"
              onClick={() => onUpdateUserData(ability, { favorite: !ability.user.favorite })}
              type="button"
            >
              {ability.user.favorite ? "取消收藏" : "收藏能力"}
            </button>
            <button
              className="action"
              onClick={() => onUpdateUserData(ability, { hidden: !ability.user.hidden })}
              type="button"
            >
              {ability.user.hidden ? "取消隐藏" : "隐藏能力"}
            </button>
          </div>
          <EditableMetadata
            ability={ability}
            onSave={(patch) => onUpdateUserData(ability, patch)}
          />
        </div>
      ) : null}
    </div>
  );
}
