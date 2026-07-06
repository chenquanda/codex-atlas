import { describe, expect, it, vi } from "vitest";
import { createAtlasApi } from "./atlasApi";
import type { TauriInvoker } from "./tauriBridge";

function createInvokeMock(response: unknown) {
  return vi.fn(
    async <T>(_command: string, _args?: Record<string, unknown>) => response as T
  ) as TauriInvoker;
}

describe("createAtlasApi", () => {
  it("passes relative_path to readSkillFile", async () => {
    const invoke = createInvokeMock({
      skill_id: "skill:test",
      relative_path: "references/guide.md",
      content: "",
      size_bytes: 0,
      language: "Other"
    });
    const api = createAtlasApi(invoke);

    await api.readSkillFile("skill:test", "references/guide.md");

    expect(invoke).toHaveBeenCalledWith("read_skill_file", {
      id: "skill:test",
      relative_path: "references/guide.md"
    });
  });

  it("passes relative_path to getTranslationState", async () => {
    const invoke = createInvokeMock({
      skill_id: "skill:test",
      relative_path: "SKILL.md",
      content_hash: "hash",
      show_translate_action: true,
      cached_translation: null
    });
    const api = createAtlasApi(invoke);

    await api.getTranslationState("skill:test", "SKILL.md");

    expect(invoke).toHaveBeenCalledWith("get_translation_state", {
      id: "skill:test",
      relative_path: "SKILL.md"
    });
  });

  it("passes relative_path to translateSkillFile", async () => {
    const invoke = createInvokeMock({
      skill_id: "skill:test",
      relative_path: "SKILL.md",
      content_hash: "hash",
      translation: "译文",
      cached: false
    });
    const api = createAtlasApi(invoke);

    await api.translateSkillFile("skill:test", "SKILL.md");

    expect(invoke).toHaveBeenCalledWith("translate_skill_file", {
      id: "skill:test",
      relative_path: "SKILL.md"
    });
  });
});
