import * as React from "react";

import { RiArrowLeftDoubleLine, RiArrowRightDoubleLine, RiCommandLine } from "@remixicon/react";

import { appNavigation, appSecondaryNavigation } from "@/app/navigation";
import { NavMain } from "@/components/nav-main";
import { NavSecondary } from "@/components/nav-secondary";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  SidebarTrigger,
  useSidebar,
} from "@/components/ui/sidebar";

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const { isMobile, state, toggleSidebar } = useSidebar();
  const isCollapsed = !isMobile && state === "collapsed";
  const toggleLabel = isCollapsed ? "Expand sidebar" : "Collapse sidebar";

  return (
    <Sidebar variant="inset" collapsible="icon" {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <div className="flex items-center gap-2">
              <SidebarTrigger
                aria-label={toggleLabel}
                title={toggleLabel}
                className="group/trigger size-9 shrink-0 bg-sidebar-primary text-sidebar-primary-foreground hover:bg-sidebar-primary/90"
              >
                <span className="flex size-full items-center justify-center">
                  <RiCommandLine
                    aria-hidden="true"
                    className="size-4 group-hover/sidebar:hidden group-focus-visible/trigger:hidden"
                  />
                  {isCollapsed ? (
                    <RiArrowLeftDoubleLine
                      aria-hidden="true"
                      className="hidden size-4 group-hover/sidebar:block group-focus-visible/trigger:block"
                    />
                  ) : (
                    <RiArrowRightDoubleLine
                      aria-hidden="true"
                      className="hidden size-4 group-hover/sidebar:block group-focus-visible/trigger:block"
                    />
                  )}
                </span>
              </SidebarTrigger>
              <SidebarMenuButton
                size="lg"
                className="min-w-0 flex-1 bg-transparent p-0! hover:bg-transparent active:bg-transparent data-active:bg-transparent data-open:hover:bg-transparent group-data-[collapsible=icon]:hidden"
                onClick={toggleSidebar}
              >
                <span className="flex min-w-0 flex-col gap-0.5 text-left">
                  <span className="truncate font-heading text-sm font-medium">YssSkills</span>
                  <span className="truncate text-[0.65rem] text-sidebar-foreground/70">
                    Skills Manager
                  </span>
                </span>
              </SidebarMenuButton>
            </div>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <nav aria-label="Primary navigation">
          <NavMain items={appNavigation} />
        </nav>
      </SidebarContent>
      <SidebarFooter>
        <nav aria-label="Secondary navigation">
          <NavSecondary items={appSecondaryNavigation} className="p-0" />
        </nav>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
