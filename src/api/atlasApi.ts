import { tauriInvoke, type TauriInvoker } from "./tauriBridge";

export type AbilityKind = "Skill" | "Plugin" | "Tool" | "App";

export interface AIData {
  summary_zh: string | null;
  tags: string[];
  call_template: string | null;
  scenarios: string[];
}

export interface UserData {
  alias: string | null;
  tags: string[];
  tags_overridden: boolean;
  note: string | null;
  favorite: boolean;
  hidden: boolean;
  custom_template: string | null;
}

export interface Stats {
  usage_count: number | null;
  last_used_at: string | null;
}

export interface Ability {
  id: string;
  name: string;
  kind: AbilityKind;
  path: string | null;
  summary: string;
  raw_tags: string[];
  ai: AIData;
  user: UserData;
  stats: Stats;
}

export interface UserDataPatch {
  alias?: string | null;
  tags?: string[];
  tags_overridden?: boolean;
  note?: string | null;
  favorite?: boolean;
  hidden?: boolean;
  custom_template?: string | null;
}

export interface ScanSummary {
  ability_count: number;
  warning_count: number;
  warnings: string[];
}

export interface StatsSummary {
  ability_count: number;
  warning_count: number;
  warnings: string[];
  complete: boolean;
  saved_cache: boolean;
}

export interface SkillDetailWindowInfo {
  skill_id: string;
  title: string;
  root_path: string;
}

export interface SkillFileEntry {
  relative_path: string;
  size_bytes: number;
  extension: string | null;
}

export interface SkillFileList {
  skill_id: string;
  root_path: string;
  files: SkillFileEntry[];
  warnings: string[];
}

export type SkillFileLanguage = "Chinese" | "Other";

export interface SkillFileContent {
  skill_id: string;
  relative_path: string;
  content: string;
  size_bytes: number;
  language: SkillFileLanguage;
}

export interface TranslationState {
  skill_id: string;
  relative_path: string;
  content_hash: string;
  show_translate_action: boolean;
  cached_translation: string | null;
}

export interface TranslationResult {
  skill_id: string;
  relative_path: string;
  content_hash: string;
  translation: string;
  cached: boolean;
}

export interface TranslationCommandError {
  kind: string;
  message: string;
  status_code: number | null;
  stdout: string | null;
  stderr: string | null;
  timeout_millis: number | null;
}

export interface AtlasApi {
  listAbilities: () => Promise<Ability[]>;
  updateUserData: (id: string, patch: UserDataPatch) => Promise<Ability>;
  refreshScan: () => Promise<ScanSummary>;
  refreshStats: () => Promise<StatsSummary>;
  copyCallTemplate: (id: string) => Promise<string>;
  openSkillDetailWindow: (id: string) => Promise<SkillDetailWindowInfo>;
  listSkillFiles: (id: string) => Promise<SkillFileList>;
  readSkillFile: (id: string, relativePath: string) => Promise<SkillFileContent>;
  getTranslationState: (id: string, relativePath: string) => Promise<TranslationState>;
  translateSkillFile: (id: string, relativePath: string) => Promise<TranslationResult>;
}

export function createAtlasApi(invokeCommand: TauriInvoker = tauriInvoke): AtlasApi {
  return {
    listAbilities: () => invokeCommand<Ability[]>("list_abilities"),
    updateUserData: (id, patch) =>
      invokeCommand<Ability>("update_user_data", {
        id,
        patch
      }),
    refreshScan: () => invokeCommand<ScanSummary>("refresh_scan"),
    refreshStats: () => invokeCommand<StatsSummary>("refresh_stats"),
    copyCallTemplate: (id) =>
      invokeCommand<string>("copy_call_template", {
        id
      }),
    openSkillDetailWindow: (id) =>
      invokeCommand<SkillDetailWindowInfo>("open_skill_detail_window", {
        id
      }),
    listSkillFiles: (id) =>
      invokeCommand<SkillFileList>("list_skill_files", {
        id
      }),
    readSkillFile: (id, relativePath) =>
      invokeCommand<SkillFileContent>("read_skill_file", {
        id,
        relative_path: relativePath
      }),
    getTranslationState: (id, relativePath) =>
      invokeCommand<TranslationState>("get_translation_state", {
        id,
        relative_path: relativePath
      }),
    translateSkillFile: (id, relativePath) =>
      invokeCommand<TranslationResult>("translate_skill_file", {
        id,
        relative_path: relativePath
      })
  };
}

export const atlasApi = createAtlasApi();
