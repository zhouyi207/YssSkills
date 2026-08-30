import { RiRefreshLine } from "@remixicon/react";

import { Button } from "@/components/ui/button";
import { Card, CardAction, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

const stats = [
  {
    label: "Skills",
    value: "24",
  },
  {
    label: "Skills Set",
    value: "2",
  },
  {
    label: "Agents",
    value: "3",
  },
  {
    label: "Projects",
    value: "3",
  },
] as const;

export function SectionCards() {
  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      {stats.map((stat) => (
        <Card key={stat.label} className="border ring-0!">
          <CardHeader>
            <CardDescription>{stat.label}</CardDescription>
            <CardTitle className="text-2xl font-semibold tabular-nums">{stat.value}</CardTitle>
            <CardAction>
              <Button type="button" variant="outline" size="sm">
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
