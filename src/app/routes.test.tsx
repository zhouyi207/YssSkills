import { describe, expect, it } from "vitest";
import { createMemoryRouter } from "react-router";

import { appRouteConfig } from "./route-config";

describe("app route config", () => {
  it("registers every dashboard page", () => {
    const layoutRoute = appRouteConfig.find((route) => route.children);
    const paths = layoutRoute?.children
      ?.map((route) => route.path)
      .filter((path): path is string => typeof path === "string");

    expect(paths).toEqual(["/dashboard", "/skills", "/workspaces", "/registry", "/settings"]);
  });

  it("redirects the root path to the dashboard", async () => {
    const router = createMemoryRouter(appRouteConfig, {
      initialEntries: ["/dashboard"],
    });

    await router.navigate("/");

    expect(router.state.location.pathname).toBe("/dashboard");
  });

  it("redirects unknown paths to the dashboard", async () => {
    const router = createMemoryRouter(appRouteConfig, {
      initialEntries: ["/dashboard"],
    });

    await router.navigate("/missing");

    expect(router.state.location.pathname).toBe("/dashboard");
  });
});
