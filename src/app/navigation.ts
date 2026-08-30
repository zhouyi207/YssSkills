import {
  RiBarChartLine,
  RiDashboardLine,
  RiFolderLine,
  RiGroupLine,
  RiSettingsLine,
} from "@remixicon/react";

export const defaultRoutePath = "/dashboard";

export const appNavigation = [
  {
    title: "Overview",
    href: "/dashboard",
    icon: RiDashboardLine,
  },
  {
    title: "Skills",
    href: "/skills",
    icon: RiFolderLine,
  },
  {
    title: "Workspaces",
    href: "/workspaces",
    icon: RiGroupLine,
  },
  {
    title: "Registry",
    href: "/registry",
    icon: RiBarChartLine,
  }
] as const;

export const appSecondaryNavigation = [
  {
    title: "Settings",
    href: "/settings",
    icon: RiSettingsLine,
  },
] as const;

export function normalizeRoutePath(pathname: string) {
  const normalizedPath = pathname.replace(/\/+$/, "");
  return normalizedPath || "/";
}

export function getRouteTitle(pathname: string) {
  const normalizedPath = normalizeRoutePath(pathname);
  return (
    [...appNavigation, ...appSecondaryNavigation].find((item) => item.href === normalizedPath)
      ?.title ?? "Overview"
  );
}
