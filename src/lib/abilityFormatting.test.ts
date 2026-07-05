import { describe, expect, it } from "vitest";
import { formatUsageLabel } from "./abilityFormatting";

describe("abilityFormatting", () => {
  it("renders unknown usage as 未统计", () => {
    expect(formatUsageLabel(null)).toBe("未统计");
    expect(formatUsageLabel(undefined)).toBe("未统计");
    expect(formatUsageLabel(0)).toBe("0 使用");
    expect(formatUsageLabel(7)).toBe("7 使用");
  });
});
