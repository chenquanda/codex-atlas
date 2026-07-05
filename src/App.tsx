import { useEffect, useMemo, useState } from "react";
import type {
  Ability,
  AtlasApi,
  SkillDetailWindowInfo,
  UserDataPatch
} from "./api/atlasApi";
import { atlasApi } from "./api/atlasApi";
import {
  writeClipboardText as defaultWriteClipboardText,
  type ClipboardWriter
} from "./api/clipboard";
import { AbilityList } from "./components/AbilityList";
import { AbilityToolbar } from "./components/AbilityToolbar";
import { BottomPanel } from "./components/BottomPanel";
import { DetailWindow } from "./components/DetailWindow";
import { FilterBar } from "./components/FilterBar";
import { MetricCards } from "./components/MetricCards";
import { SearchBox } from "./components/SearchBox";
import {
  type AbilityFilterState,
  type AbilitySort,
  filterAbilities,
  type KindFilter
} from "./lib/abilityFilters";
import { uniqueAbilityTags } from "./lib/abilityFormatting";
import { isTauriRuntime } from "./lib/runtime";

interface AppProps {
  api?: AtlasApi;
  writeClipboardText?: ClipboardWriter;
}

const initialFilters: AbilityFilterState = {
  query: "",
  kind: "Skill",
  favoriteOnly: false,
  hiddenOnly: false,
  tag: "",
  sortBy: "usage"
};

export default function App({
  api = atlasApi,
  writeClipboardText = defaultWriteClipboardText
}: AppProps) {
  const [abilities, setAbilities] = useState<Ability[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filters, setFilters] = useState<AbilityFilterState>(initialFilters);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [copyStatus, setCopyStatus] = useState<string | null>(null);
  const [moreOpen, setMoreOpen] = useState(false);
  const [detailAbility, setDetailAbility] = useState<Ability | null>(null);
  const [detailInfo, setDetailInfo] = useState<SkillDetailWindowInfo | null>(null);

  useEffect(() => {
    let active = true;

    setLoading(true);
    api
      .listAbilities()
      .then((nextAbilities) => {
        if (!active) {
          return;
        }
        setAbilities(nextAbilities);
        setError(null);
      })
      .catch((reason: unknown) => {
        if (!active) {
          return;
        }
        setError(errorMessage(reason));
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [api]);

  useEffect(() => {
    if (detailAbility || abilities.length === 0) {
      return;
    }

    const requestedId = consumeSkillDetailParam();
    if (!requestedId) {
      return;
    }

    const requestedAbility = abilities.find((ability) => ability.id === requestedId);
    if (!requestedAbility) {
      return;
    }

    setSelectedId(requestedAbility.id);
    setDetailInfo(null);
    setDetailAbility(requestedAbility);
  }, [abilities, detailAbility]);

  const filteredAbilities = useMemo(
    () => filterAbilities(abilities, filters),
    [abilities, filters]
  );
  const tagOptions = useMemo(() => uniqueAbilityTags(abilities), [abilities]);
  const selectedAbility =
    filteredAbilities.find((ability) => ability.id === selectedId) ?? filteredAbilities[0] ?? null;

  useEffect(() => {
    if (selectedAbility && selectedAbility.id !== selectedId) {
      setSelectedId(selectedAbility.id);
      setMoreOpen(false);
      setCopyStatus(null);
    }
  }, [selectedAbility, selectedId]);

  function updateFilters(patch: Partial<AbilityFilterState>) {
    setFilters((current) => ({
      ...current,
      ...patch
    }));
    setMoreOpen(false);
  }

  async function reloadAbilities() {
    const nextAbilities = await api.listAbilities();
    setAbilities(nextAbilities);
  }

  async function handleRefreshScan() {
    setError(null);
    setStatus(null);
    setCopyStatus(null);
    try {
      setStatus("正在刷新扫描");
      await api.refreshScan();
      await reloadAbilities();
      setStatus("扫描已刷新");
    } catch (reason) {
      setStatus(null);
      setCopyStatus(null);
      setError(errorMessage(reason));
    }
  }

  async function handleRefreshStats() {
    setError(null);
    setStatus(null);
    setCopyStatus(null);
    try {
      setStatus("正在刷新统计");
      await api.refreshStats();
      await reloadAbilities();
      setStatus("统计已刷新");
    } catch (reason) {
      setStatus(null);
      setCopyStatus(null);
      setError(errorMessage(reason));
    }
  }

  async function handleUpdateUserData(ability: Ability, patch: UserDataPatch) {
    setError(null);
    setStatus(null);
    try {
      const nextAbility = await api.updateUserData(ability.id, patch);
      setAbilities((current) =>
        current.map((item) => (item.id === nextAbility.id ? nextAbility : item))
      );
      setStatus("元数据已保存");
    } catch (reason) {
      setStatus(null);
      setError(errorMessage(reason));
    }
  }

  async function handleCopy(ability: Ability) {
    setError(null);
    setStatus(null);
    setCopyStatus(null);
    try {
      const template = await api.copyCallTemplate(ability.id);
      await writeClipboardText(template);
      setCopyStatus(`已复制 ${template}`);
      setStatus("调用模板已复制");
    } catch (reason) {
      setStatus(null);
      setCopyStatus(null);
      setError(errorMessage(reason));
    }
  }

  async function handleOpenDetail(ability: Ability) {
    if (ability.kind !== "Skill") {
      return;
    }

    setError(null);
    setStatus(null);
    setCopyStatus(null);
    try {
      const info = await api.openSkillDetailWindow(ability.id);
      if (!isTauriRuntime()) {
        setDetailInfo(info);
        setDetailAbility(ability);
      }
    } catch (reason) {
      setDetailInfo(null);
      setError(errorMessage(reason));
    }
  }

  return (
    <main className="atlas-page" aria-label="Codex Atlas">
      <section className="window atlas" aria-label="Codex Atlas 主窗口">
        <div className="window-bar">
          <span className="dot" aria-hidden="true" />
          <span className="dot" aria-hidden="true" />
          <span className="dot" aria-hidden="true" />
          <span className="bar-title">Codex Atlas</span>
          <span className="bar-kbd">Ctrl Alt Space</span>
        </div>

        <div className="atlas-body">
          <div className="topline">
            <div className="brand-row">
              <div className="brand-mark">CA</div>
              <div className="brand-copy">
                <h1>能力索引</h1>
                <span>搜索、阅读、整理 Codex abilities</span>
              </div>
            </div>
            <div className="top-actions">
              <button className="small-btn" onClick={handleRefreshScan} type="button">
                扫描
              </button>
              <button className="small-btn" onClick={handleRefreshStats} type="button">
                统计
              </button>
            </div>
          </div>

          <MetricCards abilities={abilities} />

          <div className="search-row">
            <SearchBox value={filters.query} onChange={(query) => updateFilters({ query })} />
            <AbilityToolbar
              value={filters.kind}
              onChange={(kind: KindFilter) => updateFilters({ kind })}
            />
          </div>

          <FilterBar
            favoriteOnly={filters.favoriteOnly}
            hiddenOnly={filters.hiddenOnly}
            tag={filters.tag}
            tagOptions={tagOptions}
            sortBy={filters.sortBy}
            onFavoriteOnlyChange={(favoriteOnly) => updateFilters({ favoriteOnly })}
            onHiddenOnlyChange={(hiddenOnly) => updateFilters({ hiddenOnly })}
            onTagChange={(tag) => updateFilters({ tag })}
            onSortChange={(sortBy: AbilitySort) => updateFilters({ sortBy })}
          />

          {error ? <div className="notice error">{error}</div> : null}
          {status ? (
            <div className="notice" role="status">
              {status}
            </div>
          ) : null}

          <AbilityList
            abilities={filteredAbilities}
            loading={loading}
            selectedId={selectedAbility?.id ?? null}
            onSelect={(ability) => {
              setSelectedId(ability.id);
              setMoreOpen(false);
              setCopyStatus(null);
            }}
          />

          <BottomPanel
            ability={selectedAbility}
            copyStatus={copyStatus}
            moreOpen={moreOpen}
            onCopy={handleCopy}
            onOpenDetail={handleOpenDetail}
            onMoreChange={setMoreOpen}
            onUpdateUserData={handleUpdateUserData}
          />
        </div>
      </section>

      {detailAbility ? (
        <DetailWindow
          ability={detailAbility}
          api={api}
          detailInfo={detailInfo}
          onClose={() => {
            setDetailInfo(null);
            setDetailAbility(null);
          }}
        />
      ) : null}
    </main>
  );
}

function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}

function consumeSkillDetailParam(): string | null {
  const url = new URL(window.location.href);
  const requestedId = url.searchParams.get("skillDetail");
  if (!requestedId) {
    return null;
  }

  url.searchParams.delete("skillDetail");
  const search = url.searchParams.toString();
  const nextUrl = `${url.pathname}${search ? `?${search}` : ""}${url.hash}`;
  window.history.replaceState(window.history.state, "", nextUrl);

  return requestedId;
}
