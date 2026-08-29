import type { ReactNode } from "react";

import { Badge } from "@/components/ui/badge";

export function PageHeader({
  eyebrow,
  title,
  description,
  action,
}: {
  eyebrow: string;
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-4 border-b px-4 py-6 lg:flex-row lg:items-end lg:justify-between lg:px-6">
      <div className="flex max-w-3xl flex-col gap-2">
        <Badge variant="outline" className="w-fit uppercase tracking-[0.18em]">
          {eyebrow}
        </Badge>
        <div className="flex flex-col gap-1">
          <h2 className="font-heading text-2xl font-medium tracking-tight">{title}</h2>
          <p className="text-sm text-muted-foreground">{description}</p>
        </div>
      </div>
      {action ? <div className="shrink-0">{action}</div> : null}
    </div>
  );
}
