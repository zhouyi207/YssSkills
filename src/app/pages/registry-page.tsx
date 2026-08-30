import { useEffect, useMemo, useState } from "react";
import { RiDownloadLine, RiExternalLinkLine, RiRefreshLine, RiSearchLine } from "@remixicon/react";
import { toast } from "sonner";

import { useRegistry } from "@/app/hooks/use-registry";
import { registryService } from "@/app/services/registry-service";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { IpcError } from "@/shared/types/ipc";
import type { RegistrySkillSummaryDto } from "@/shared/types/registry";

const installCountFormatter = new Intl.NumberFormat();

function registrySkillKey(skill: RegistrySkillSummaryDto) {
  return `${skill.id.source}\0${skill.id.skillId}`;
}

async function openRegistryDetails(detailsUrl: string) {
  try {
    await registryService.openDetails(detailsUrl);
  } catch {
    toast.error("Unable to open registry details.");
  }
}

function retryAvailableAt(error: IpcError | null) {
  if (!error?.retryAfter) {
    return null;
  }

  return error.retryAfter.kind === "delay"
    ? Date.now() + error.retryAfter.seconds * 1_000
    : error.retryAfter.epochMillis;
}

function RegistrySkillList({ items }: { items: RegistrySkillSummaryDto[] }) {
  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {items.map((skill) => {
        const detailsUrl = skill.detailsUrl?.trim() || null;
        const formattedInstalls = installCountFormatter.format(skill.installs);

        return (
          <div
            key={registrySkillKey(skill)}
            role="listitem"
            className="group/registry-row mx-4 grid min-h-14 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_auto_2rem] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50"
          >
            <div className="flex size-6 items-center justify-center">
              <Checkbox
                checked={false}
                disabled
                aria-label={`Installation unavailable for ${skill.name}`}
                title="Registry installation is not available yet"
                className="opacity-0 transition-opacity group-hover/registry-row:opacity-100 group-focus-within/registry-row:opacity-100"
              />
            </div>
            <div className="flex min-w-0 items-baseline gap-2">
              <h3 className="min-w-0 truncate font-medium">{skill.name}</h3>
              <span
                className="max-w-36 shrink-0 truncate text-[0.65rem] text-muted-foreground"
                title={skill.id.source}
              >
                {skill.id.source}
              </span>
            </div>
            <div className="shrink-0 justify-self-end">
              <Badge variant="secondary" title={`${formattedInstalls} installs`}>
                {formattedInstalls}
              </Badge>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              disabled={detailsUrl === null}
              aria-label={
                detailsUrl
                  ? `Open details for ${skill.name}`
                  : `Details unavailable for ${skill.name}`
              }
              title={detailsUrl ? `Open details for ${skill.name}` : "Details unavailable"}
              className="justify-self-end"
              onClick={(event) => {
                event.stopPropagation();
                if (detailsUrl) {
                  void openRegistryDetails(detailsUrl);
                }
              }}
            >
              <RiExternalLinkLine aria-hidden="true" />
            </Button>
          </div>
        );
      })}
    </div>
  );
}

export function RegistryPage() {
  const [query, setQuery] = useState("");
  const [retryClock, setRetryClock] = useState(() => Date.now());
  const { data, error, isLoading, isRefreshing, search, refresh, retry } = useRegistry();
  const retryAt = useMemo(() => retryAvailableAt(error), [error]);

  useEffect(() => {
    if (retryAt === null) {
      return;
    }

    const now = Date.now();
    setRetryClock(now);
    if (retryAt <= now) {
      return;
    }

    const timeout = window.setTimeout(() => setRetryClock(Date.now()), retryAt - now);
    return () => window.clearTimeout(timeout);
  }, [retryAt]);

  const isBusy = isLoading || isRefreshing;
  const isRetryDeferred = retryAt !== null && retryAt > retryClock;
  const retryDelaySeconds = isRetryDeferred
    ? Math.max(1, Math.ceil((retryAt - retryClock) / 1_000))
    : null;
  const canRetry = error?.retryable === true && !isRetryDeferred;
  const isQueryValidationError = error?.code === "registry.invalid_query";
  const isRefreshBlocked =
    isRetryDeferred || (error !== null && !error.retryable && !isQueryValidationError);

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Registry</h1>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled
            title="Installing registry skills is not available yet"
          >
            <RiDownloadLine aria-hidden="true" data-icon="inline-start" />
            Install unavailable
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={isBusy || isRefreshBlocked}
            aria-busy={isRefreshing}
            onClick={() => void refresh()}
          >
            <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
            Refresh
          </Button>
        </div>
      </header>

      <div
        className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border bg-background"
        aria-busy={isBusy}
      >
        <form
          className="flex w-full items-center gap-2 p-4"
          onSubmit={(event) => {
            event.preventDefault();
            void search(query);
          }}
        >
          <div className="relative min-w-0 flex-1">
            <RiSearchLine
              aria-hidden="true"
              className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
            />
            <Input
              id="registry-search"
              aria-label="Search registry skills"
              aria-describedby={error ? "registry-error-message" : undefined}
              aria-invalid={isQueryValidationError}
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder="Search registry skills"
              className="pl-8"
            />
          </div>
          <Button type="submit" disabled={isBusy || isRetryDeferred}>
            <RiSearchLine aria-hidden="true" data-icon="inline-start" />
            Search
          </Button>
        </form>

        {isLoading && data === null ? (
          <div className="px-4 py-10 text-center" role="status" aria-live="polite">
            <p className="text-sm font-medium">Loading registry</p>
            <p className="mt-1 text-sm text-muted-foreground">Fetching the all-time leaderboard.</p>
          </div>
        ) : error ? (
          <div id="registry-error-message" className="px-4 py-10 text-center" role="alert">
            <p className="text-sm font-medium">
              {isQueryValidationError ? "Search query is too short" : "Unable to load registry"}
            </p>
            <p className="mt-1 text-sm text-muted-foreground">{error.message}</p>
            {retryDelaySeconds !== null ? (
              <p className="mt-1 text-sm text-muted-foreground">
                Retry available in {installCountFormatter.format(retryDelaySeconds)} seconds.
              </p>
            ) : null}
            {canRetry ? (
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="mt-4"
                onClick={() => void retry()}
              >
                <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                Retry
              </Button>
            ) : null}
          </div>
        ) : data && data.skills.length > 0 ? (
          <ScrollArea className="min-h-0 min-w-0 flex-1">
            <RegistrySkillList items={data.skills} />
          </ScrollArea>
        ) : data ? (
          <div className="px-4 py-10 text-center" aria-live="polite">
            <p className="text-sm font-medium">
              {data.mode === "search" ? "No matching results" : "No registry skills available"}
            </p>
            <p className="mt-1 text-sm text-muted-foreground">
              {data.mode === "search"
                ? "Try a different search term."
                : "The all-time leaderboard is currently empty."}
            </p>
          </div>
        ) : (
          <div className="px-4 py-10 text-center">
            <p className="text-sm font-medium">Registry data is unavailable</p>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="mt-4"
              onClick={() => void refresh()}
            >
              <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
              Retry
            </Button>
          </div>
        )}
      </div>
    </>
  );
}
