import { describe, expect, it } from "vitest";

import {
  appNavigation,
  appSecondaryNavigation,
  defaultRoutePath,
  getRouteTitle,
} from "./navigation";

describe("app navigation", () => {
  it("exposes the primary dashboard routes in a stable order", () => {
    expect(appNavigation.map((item) => item.href)).toEqual([
      "/dashboard",
      "/skills",
      "/workspaces",
      "/registry",
    ]);
  });

  it("exposes settings as secondary navigation", () => {
    expect(appSecondaryNavigation.map((item) => item.href)).toEqual(["/settings"]);
  });

  it("uses the dashboard as the default route", () => {
    expect(defaultRoutePath).toBe("/dashboard");
  });

  it("falls back to the overview title for an unknown route", () => {
    expect(getRouteTitle("/does-not-exist")).toBe("Overview");
  });

  it("keeps route titles when the URL has a trailing slash", () => {
    expect(getRouteTitle("/skills/")).toBe("Skills");
  });
});
