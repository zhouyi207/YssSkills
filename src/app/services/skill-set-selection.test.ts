import { describe, expect, it } from "vitest";

import { applySkillSetSelection } from "./skill-set-selection";

describe("Skill Set selection", () => {
  it("adds every available member and returns to the item tab", () => {
    const result = applySkillSetSelection(
      new Set(["already-selected"]),
      {
        id: "set-1",
        name: "Daily tools",
        skillIds: ["alpha", "beta", "missing"],
      },
      new Set(["already-selected", "alpha", "beta"]),
    );

    expect(Array.from(result.selectedSkillIds)).toEqual(["already-selected", "alpha", "beta"]);
    expect(result.activeTab).toBe("item");
  });
});
