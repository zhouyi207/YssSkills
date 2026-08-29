import { ThemeProvider } from "next-themes";
import { Outlet, redirect } from "react-router";

import { AppSidebar } from "@/components/app-sidebar";

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { Toaster } from "@/components/ui/sonner";

import { TooltipProvider } from "@/components/ui/tooltip";
import { defaultRoutePath } from "./navigation";
import { DashboardPage } from "./pages/dashboard-page";
import { RegistryPage } from "./pages/registry-page";
import { SettingsPage } from "./pages/settings-page";
import { SkillsPage } from "./pages/skills-page";
import { WorkspacesPage } from "./pages/workspaces-page";

function AppLayout() {
  return (
    <TooltipProvider>
      <ThemeProvider attribute="class" defaultTheme="light" enableSystem={false}>
        <SidebarProvider className="h-svh max-h-svh overflow-hidden">
          <AppSidebar />
          <SidebarInset className="h-full min-h-0 min-w-0 overflow-x-hidden">
            <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col gap-2 overflow-hidden bg-muted/20 p-2">
              <Outlet />
            </div>
          </SidebarInset>
        </SidebarProvider>
        <Toaster />
      </ThemeProvider>
    </TooltipProvider>
  );
}

export const appRouteConfig = [
  {
    path: "/",
    loader: () => redirect(defaultRoutePath),
  },
  {
    element: <AppLayout />,
    children: [
      {
        path: "/dashboard",
        element: <DashboardPage />,
      },
      {
        path: "/skills",
        element: <SkillsPage />,
      },
      {
        path: "/workspaces",
        element: <WorkspacesPage />,
      },
      {
        path: "/registry",
        element: <RegistryPage />,
      },
      {
        path: "/settings",
        element: <SettingsPage />,
      },
    ],
  },
  {
    path: "*",
    loader: () => redirect(defaultRoutePath),
  },
];
