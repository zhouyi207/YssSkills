import { RiRefreshLine } from "@remixicon/react";

import { useDashboardOverview } from "@/app/hooks/use-dashboard-overview";
import { ChartAreaInteractive } from "@/components/chart-area-interactive";
import { IpcErrorDetails } from "@/components/ipc-error-details";
import { SectionCards } from "@/components/section-cards";
import { Button } from "@/components/ui/button";

export function DashboardPage() {
  const { data, error, isLoading, isRefreshing, refresh } = useDashboardOverview();

  const handleRefresh = () => {
    void refresh();
  };

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Overview</h1>
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-busy={isRefreshing}
          disabled={isRefreshing}
          onClick={handleRefresh}
        >
          <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
          Refresh
        </Button>
      </header>

      {isLoading ? (
        <div
          role="status"
          aria-live="polite"
          className="flex shrink-0 items-center justify-between"
        >
          <p className="font-heading text-sm font-medium">Loading overview…</p>
        </div>
      ) : null}

      {!isLoading && error && !data ? (
        <div role="alert" className="flex shrink-0 items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="font-heading text-sm font-medium">Unable to load the overview</p>
            <IpcErrorDetails error={error} compact />
          </div>
          <Button type="button" variant="outline" size="sm" onClick={handleRefresh}>
            <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
            Retry
          </Button>
        </div>
      ) : null}

      {!isLoading && data && error ? (
        <div role="alert" className="flex shrink-0 items-start justify-between gap-4">
          <div className="min-w-0">
            <p className="font-heading text-sm font-medium">Unable to refresh the overview</p>
            <IpcErrorDetails error={error} compact />
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={isRefreshing}
            onClick={handleRefresh}
          >
            <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
            Retry
          </Button>
        </div>
      ) : null}

      {!isLoading && data ? (
        <SectionCards counts={data.counts} isRefreshing={isRefreshing} onRefresh={handleRefresh} />
      ) : null}

      {!isLoading && data ? <ChartAreaInteractive activity={data.activity} /> : null}
    </>
  );
}
