import * as React from "react";
import { Area, AreaChart, CartesianGrid, XAxis } from "recharts";

import { useIsMobile } from "@/hooks/use-mobile";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { DashboardActivityDto } from "@/shared/types/dashboard";

const chartConfig = {
  installed: {
    label: "Installed",
    color: "var(--chart-1)",
  },
  updated: {
    label: "Updated",
    color: "var(--chart-2)",
  },
} satisfies ChartConfig;

export function ChartAreaInteractive({ activity }: { activity: DashboardActivityDto[] }) {
  const isMobile = useIsMobile();
  const [timeRange, setTimeRange] = React.useState("12w");

  React.useEffect(() => {
    if (isMobile) {
      setTimeRange("4w");
    }
  }, [isMobile]);

  const activityData = React.useMemo(
    () =>
      activity.map((period) => ({
        week: new Intl.DateTimeFormat("en-US", {
          month: "short",
          day: "2-digit",
          timeZone: "UTC",
        }).format(new Date(period.periodStartEpochMillis)),
        installed: period.imported,
        updated: period.updated,
      })),
    [activity],
  );
  const visibleData = timeRange === "4w" ? activityData.slice(-4) : activityData;

  return (
    <Card className="@container/card min-h-0 min-w-0 flex-1 border ring-0!">
      <CardHeader>
        <CardTitle>Skill activity</CardTitle>
        <CardDescription>
          Installs and updates across the current workspace network.
        </CardDescription>
        <CardAction>
          <ToggleGroup
            type="single"
            value={timeRange}
            onValueChange={(value) => {
              if (value) {
                setTimeRange(value);
              }
            }}
            variant="outline"
            className="hidden *:data-[slot=toggle-group-item]:px-4! @[767px]/card:flex"
          >
            <ToggleGroupItem value="12w">12 weeks</ToggleGroupItem>
            <ToggleGroupItem value="4w">4 weeks</ToggleGroupItem>
          </ToggleGroup>
          <Select value={timeRange} onValueChange={setTimeRange}>
            <SelectTrigger
              className="flex w-32 @[767px]/card:hidden"
              size="sm"
              aria-label="Select a time range"
            >
              <SelectValue placeholder="12 weeks" />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="12w">12 weeks</SelectItem>
                <SelectItem value="4w">4 weeks</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </CardAction>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col px-2 pt-4 sm:px-6 sm:pt-6">
        <ChartContainer
          config={chartConfig}
          className="aspect-auto flex min-h-0 min-w-0 w-full flex-1"
        >
          <AreaChart accessibilityLayer data={visibleData}>
            <defs>
              <linearGradient id="fillInstalled" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="var(--color-installed)" stopOpacity={0.8} />
                <stop offset="95%" stopColor="var(--color-installed)" stopOpacity={0.05} />
              </linearGradient>
              <linearGradient id="fillUpdated" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="var(--color-updated)" stopOpacity={0.65} />
                <stop offset="95%" stopColor="var(--color-updated)" stopOpacity={0.03} />
              </linearGradient>
            </defs>
            <CartesianGrid vertical={false} />
            <XAxis
              dataKey="week"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              minTickGap={24}
            />
            <ChartTooltip cursor={false} content={<ChartTooltipContent indicator="dot" />} />
            <Area
              dataKey="updated"
              type="natural"
              isAnimationActive={false}
              fill="url(#fillUpdated)"
              fillOpacity={0.4}
              stroke="var(--color-updated)"
              stackId="a"
            />
            <Area
              dataKey="installed"
              type="natural"
              isAnimationActive={false}
              fill="url(#fillInstalled)"
              stroke="var(--color-installed)"
              stackId="a"
            />
          </AreaChart>
        </ChartContainer>
      </CardContent>
    </Card>
  );
}
