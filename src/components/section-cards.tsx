import { RiRefreshLine } from "@remixicon/react";

import { Button } from "@/components/ui/button";
import { Card, CardAction, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardCountsDto } from "@/shared/types/dashboard";

export function SectionCards({
  counts,
  isRefreshing,
  onRefresh,
}: {
  counts: DashboardCountsDto;
  isRefreshing: boolean;
  onRefresh: () => void;
}) {
  const stats = [
    { label: "Skills", value: counts.skills },
    { label: "Skills Set", value: counts.deployments },
    { label: "Agents", value: counts.detectedHarnesses },
    { label: "Projects", value: counts.workspaces },
  ] as const;

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      {stats.map((stat) => (
        <Card key={stat.label} className="border ring-0!">
          <CardHeader>
            <CardDescription>{stat.label}</CardDescription>
            <CardTitle className="text-2xl font-semibold tabular-nums">{stat.value}</CardTitle>
            <CardAction>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={isRefreshing}
                onClick={onRefresh}
              >
                <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                Update
              </Button>
            </CardAction>
          </CardHeader>
        </Card>
      ))}
    </div>
  );
}
