import { RiRefreshLine } from "@remixicon/react";

import { Button } from "@/components/ui/button";
import { ChartAreaInteractive } from "@/components/chart-area-interactive";
import { SectionCards } from "@/components/section-cards";

export function DashboardPage() {
  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Overview</h1>
        <Button type="button" variant="outline" size="sm">
          <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
          Refresh
        </Button>
      </header>
      <SectionCards />
      <ChartAreaInteractive />
    </>
  );
}
