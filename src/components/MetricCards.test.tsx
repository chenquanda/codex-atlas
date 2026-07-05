// @vitest-environment jsdom
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { MetricCards } from "./MetricCards";
import { ability } from "../test/fixtures";

afterEach(() => {
  cleanup();
});

describe("MetricCards", () => {
  it("counts only positive usage as 已使用", () => {
    render(
      <MetricCards
        abilities={[
          ability({ id: "skill:unknown", name: "unknown", stats: { usage_count: null, last_used_at: null } }),
          ability({ id: "skill:zero", name: "zero", stats: { usage_count: 0, last_used_at: null } }),
          ability({ id: "skill:used", name: "used", stats: { usage_count: 3, last_used_at: null } })
        ]}
      />
    );

    const overview = screen.getByLabelText("能力概览");
    expect(within(overview).getByText("已使用").previousElementSibling?.textContent).toBe("1");
  });
});
