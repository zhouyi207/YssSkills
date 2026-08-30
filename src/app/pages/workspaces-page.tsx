import { useMemo, useState } from "react";
import {
  RiAddLine,
  RiCheckboxMultipleLine,
  RiEditLine,
  RiFolderLine,
  RiRefreshLine,
  RiSearchLine,
} from "@remixicon/react";
import { toast } from "sonner";

import { useWorkspaces } from "@/app/hooks/use-workspaces";
import { selectDirectory } from "@/app/services/directory-picker";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@/components/ui/combobox";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import type { IpcError } from "@/shared/types/ipc";
import type {
  HarnessSummaryDto,
  WorkspaceReconcileOutcomeDto,
  WorkspaceSummaryDto,
} from "@/shared/types/workspaces";

type WorkspaceTab = "agent" | "project";
type ProjectWorkspace = Omit<WorkspaceSummaryDto, "kind"> & {
  kind: Extract<WorkspaceSummaryDto["kind"], { kind: "project" | "linked" }>;
};
type WorkspaceListEntry = {
  id: string;
  name: string;
  path: string;
  count: number;
};
type SelectionListItem = {
  id: string;
  name: string;
  subtitle: string;
};

function isProjectWorkspace(workspace: WorkspaceSummaryDto): workspace is ProjectWorkspace {
  return workspace.kind.kind === "project" || workspace.kind.kind === "linked";
}

function getDirectoryName(path: string) {
  const segments = path
    .replace(/[\\/]+$/, "")
    .split(/[\\/]/)
    .filter(Boolean);
  return segments[segments.length - 1] ?? "Imported project";
}

function matchesQuery(query: string, values: ReadonlyArray<string | number>) {
  if (!query) {
    return true;
  }

  return values.some((value) => String(value).toLowerCase().includes(query));
}

function ErrorNotice({
  title,
  error,
  onRetry,
  isRetrying = false,
}: {
  title: string;
  error: IpcError;
  onRetry?: () => void;
  isRetrying?: boolean;
}) {
  return (
    <div className="px-4 py-10 text-center" role="alert" aria-live="polite">
      <p className="text-sm font-medium">{title}</p>
      <p className="mt-1 text-sm text-muted-foreground">{error.message}</p>
      <p className="mt-1 text-[0.65rem] text-muted-foreground">
        Error code: <code>{error.code}</code>
      </p>
      {onRetry ? (
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="mt-4"
          onClick={onRetry}
          disabled={isRetrying}
        >
          {isRetrying ? "Retrying…" : "Retry"}
        </Button>
      ) : null}
    </div>
  );
}

function EmptyState({ title, description }: { title: string; description: string }) {
  return (
    <div className="px-4 py-10 text-center" aria-live="polite">
      <p className="text-sm font-medium">{title}</p>
      <p className="mt-1 text-sm text-muted-foreground">{description}</p>
    </div>
  );
}

function WorkspaceList({
  items,
  selectedId = null,
  disabled = false,
  onSelect,
  onClearSelection,
}: {
  items: ReadonlyArray<WorkspaceListEntry>;
  selectedId?: string | null;
  disabled?: boolean;
  onSelect?: (id: string) => void | Promise<void>;
  onClearSelection?: () => void;
}) {
  const keepCheckboxesVisible = selectedId !== null;

  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {items.map((entry) => {
        const isSelected = selectedId === entry.id;
        const isSelectable = onSelect !== undefined;

        return (
          <div
            key={entry.id}
            role={isSelectable ? "button" : "listitem"}
            tabIndex={isSelectable ? 0 : undefined}
            aria-pressed={isSelectable ? isSelected : undefined}
            aria-disabled={isSelectable && disabled ? true : undefined}
            className="group/workspace-row mx-4 grid min-h-14 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_minmax(0,2fr)_auto_2rem] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50"
            onClick={() => {
              if (isSelectable && !disabled) {
                void onSelect(entry.id);
              }
            }}
            onKeyDown={(event) => {
              if (!isSelectable || disabled || event.target !== event.currentTarget) {
                return;
              }

              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                void onSelect(entry.id);
              }
            }}
          >
            <div className="flex size-6 items-center justify-center">
              <Checkbox
                aria-label={
                  isSelectable ? `Select ${entry.name}` : `Selection unavailable for ${entry.name}`
                }
                checked={isSelected}
                disabled={disabled || !isSelectable}
                className={cn(
                  "opacity-0 transition-opacity group-hover/workspace-row:opacity-100 group-focus-within/workspace-row:opacity-100",
                  keepCheckboxesVisible && "opacity-100",
                )}
                onClick={(event) => event.stopPropagation()}
                onCheckedChange={(checked) => {
                  if (!onSelect) {
                    return;
                  }

                  if (checked === true) {
                    void onSelect(entry.id);
                  } else if (isSelected) {
                    onClearSelection?.();
                  }
                }}
              />
            </div>
            <div className="flex min-w-0 items-baseline gap-2">
              <h3 className="min-w-0 truncate font-medium">{entry.name}</h3>
            </div>
            <div className="min-w-0 truncate text-muted-foreground" title={entry.path}>
              {entry.path}
            </div>
            <div className="shrink-0 justify-self-end">
              <Badge variant="secondary">{entry.count}</Badge>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={`Edit unavailable for ${entry.name}`}
              title="Workspace editing is unavailable"
              className="justify-self-end"
              disabled
              onClick={(event) => event.stopPropagation()}
            >
              <RiEditLine aria-hidden="true" />
            </Button>
          </div>
        );
      })}
    </div>
  );
}

function SelectionList({
  items,
  selectedId,
  disabled,
  onToggle,
}: {
  items: ReadonlyArray<SelectionListItem>;
  selectedId: string | null;
  disabled: boolean;
  onToggle: (id: string, checked: boolean) => void;
}) {
  return (
    <div role="list" className="flex flex-col p-1">
      {items.map((item) => (
        <label
          key={item.id}
          className="flex min-w-0 cursor-pointer items-center gap-2 px-2 py-1.5 hover:bg-muted"
        >
          <Checkbox
            aria-label={`Select ${item.name}`}
            checked={selectedId === item.id}
            disabled={disabled}
            onCheckedChange={(checked) => onToggle(item.id, checked === true)}
          />
          <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
            <span className="min-w-0 flex-1 truncate text-xs font-medium">{item.name}</span>
            <span className="shrink-0 truncate text-[0.65rem] text-muted-foreground">
              {item.subtitle}
            </span>
          </div>
        </label>
      ))}
    </div>
  );
}

function syncIssueCount(outcome: WorkspaceReconcileOutcomeDto) {
  return (
    outcome.requested.finalReport.diagnostics.length +
    outcome.propagated.reduce(
      (count, propagation) =>
        count +
        (propagation.error ? 1 : 0) +
        (propagation.report?.finalReport.diagnostics.length ?? 0),
      0,
    )
  );
}

function harnessToListEntry(harness: HarnessSummaryDto): WorkspaceListEntry {
  const path =
    harness.probe?.globalSkillsPath.display ??
    (harness.error
      ? `${harness.error.message} (${harness.error.code})`
      : "Unavailable (probe unavailable)");

  return {
    id: harness.id,
    name: harness.displayName,
    path,
    count: harness.deploymentCount,
  };
}

function projectToListEntry(workspace: ProjectWorkspace): WorkspaceListEntry {
  return {
    id: workspace.id,
    name: workspace.name,
    path: workspace.kind.root.display,
    count: workspace.deploymentCount,
  };
}

export function WorkspacesPage() {
  const {
    overview,
    error: overviewError,
    isLoading,
    isRefreshing,
    refresh,
    isObserving,
    isMutating,
    observe,
    createProject,
    reconcile,
  } = useWorkspaces();
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState<WorkspaceTab>("agent");
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [isProjectListOpen, setIsProjectListOpen] = useState(false);
  const [isSelectingProject, setIsSelectingProject] = useState(false);
  const [syncingWorkspaceId, setSyncingWorkspaceId] = useState<string | null>(null);
  const normalizedQuery = query.trim().toLowerCase();

  const projectWorkspaces = useMemo(
    () => overview?.workspaces.filter(isProjectWorkspace) ?? [],
    [overview],
  );
  const filteredHarnesses = useMemo(
    () =>
      (overview?.harnesses ?? []).filter((harness) =>
        matchesQuery(normalizedQuery, [
          harness.displayName,
          harness.probe?.globalSkillsPath.display ?? "unavailable",
          harness.probe?.detectionStatus ?? "unavailable",
          harness.deploymentCount,
          harness.error?.message ?? "",
        ]),
      ),
    [normalizedQuery, overview],
  );
  const filteredProjects = useMemo(
    () =>
      projectWorkspaces.filter((workspace) =>
        matchesQuery(normalizedQuery, [
          workspace.name,
          workspace.kind.kind,
          workspace.kind.root.display,
          workspace.deploymentCount,
          workspace.deploymentMode,
        ]),
      ),
    [normalizedQuery, projectWorkspaces],
  );
  const selectedProject = projectWorkspaces.find((workspace) => workspace.id === selectedProjectId);

  const handleTabChange = (value: string) => {
    if (value === "agent" || value === "project") {
      setActiveTab(value);
    }
  };

  const clearSelectedProject = () => {
    setSelectedProjectId(null);
  };

  const handleSelectProject = async (workspaceId: string) => {
    setSelectedProjectId(workspaceId);
    setIsProjectListOpen(true);
    const nextObservation = await observe(workspaceId);
    if (!nextObservation) {
      toast.error("Unable to observe the project workspace.");
    }
  };

  const handleProjectValueChange = (workspaceId: string | null) => {
    if (!workspaceId) {
      clearSelectedProject();
      return;
    }

    void handleSelectProject(workspaceId);
  };

  const handleAddProject = async () => {
    setIsSelectingProject(true);

    try {
      const root = await selectDirectory("Add project");

      if (!root) {
        return;
      }

      const created = await createProject(getDirectoryName(root), root);
      if (!created) {
        setIsProjectListOpen(true);
        toast.error("Unable to add the project. See the error details on this page.");
        return;
      }

      setActiveTab("project");
      setQuery("");
      setSelectedProjectId(created.id);
      setIsProjectListOpen(true);
      await observe(created.id);
      toast.success("Project added.");
    } catch {
      toast.error("Unable to open the project folder picker.");
    } finally {
      setIsSelectingProject(false);
    }
  };

  const handleRefresh = async () => {
    const selectedId = selectedProjectId;
    const nextOverview = await refresh();

    if (!nextOverview) {
      toast.error("Unable to refresh the workspace overview.");
      return;
    }

    const selectedStillExists =
      selectedId !== null &&
      nextOverview.workspaces.some(
        (workspace) => workspace.id === selectedId && isProjectWorkspace(workspace),
      );

    if (selectedId && selectedStillExists) {
      await observe(selectedId);
    } else if (selectedId) {
      clearSelectedProject();
    }

    toast.success("Workspace overview refreshed.");
  };

  const handleSync = async (workspaceId = selectedProjectId) => {
    if (!workspaceId) {
      return;
    }

    setSelectedProjectId(workspaceId);
    setSyncingWorkspaceId(workspaceId);
    setIsProjectListOpen(true);

    try {
      const outcome = await reconcile(workspaceId);
      if (!outcome) {
        toast.error("Workspace sync failed. See the error details on this page.");
        return;
      }

      const issueCount = syncIssueCount(outcome);
      if (issueCount > 0) {
        toast.warning(
          `Workspace sync completed with ${issueCount} diagnostic${issueCount === 1 ? "" : "s"}.`,
        );
      } else {
        toast.success("Workspace sync completed.");
      }
    } finally {
      setSyncingWorkspaceId(null);
    }
  };

  const isOverviewBusy = isLoading || isRefreshing;
  const isSyncingSelected = selectedProjectId !== null && syncingWorkspaceId === selectedProjectId;
  const workspaceActionsDisabled = isRefreshing || isMutating || isObserving;

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Workspaces</h1>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-label="Select all unavailable"
            title="Bulk workspace selection is unavailable"
            disabled
          >
            <RiCheckboxMultipleLine aria-hidden="true" data-icon="inline-start" />
            Select all
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-busy={isSelectingProject || isMutating}
            title="Add a project workspace"
            onClick={() => void handleAddProject()}
            disabled={isSelectingProject || isMutating}
          >
            <RiAddLine aria-hidden="true" data-icon="inline-start" />
            Add
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-haspopup="dialog"
            onClick={() => setIsProjectListOpen(true)}
          >
            <RiFolderLine aria-hidden="true" data-icon="inline-start" />
            Project list
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            title="Reload workspace data without reconciling or writing changes"
            onClick={() => void handleRefresh()}
            disabled={isOverviewBusy || isMutating}
          >
            <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
            Refresh
          </Button>
        </div>
      </header>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border bg-background">
        {isLoading && !overview ? (
          <EmptyState
            title="Loading workspaces…"
            description="Reading workspace data from the backend."
          />
        ) : !overview ? (
          overviewError ? (
            <ErrorNotice
              title="Unable to load workspaces"
              error={overviewError}
              onRetry={() => void handleRefresh()}
              isRetrying={isLoading}
            />
          ) : (
            <EmptyState
              title="Workspace overview unavailable"
              description="Refresh to load workspace data from the backend."
            />
          )
        ) : (
          <Tabs
            value={activeTab}
            onValueChange={handleTabChange}
            className="min-h-0 min-w-0 flex-1 gap-0"
          >
            <div className="flex w-full min-w-0 items-center gap-2 p-4">
              <div className="relative min-w-0 flex-1">
                <RiSearchLine
                  aria-hidden="true"
                  className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
                />
                <Input
                  aria-label="Search agents and projects"
                  value={query}
                  onChange={(event) => setQuery(event.currentTarget.value)}
                  placeholder="Search agents and projects"
                  className="pl-8"
                />
              </div>
              {activeTab === "project" ? (
                <Combobox
                  items={projectWorkspaces.map((workspace) => workspace.id)}
                  value={selectedProjectId ?? ""}
                  onValueChange={handleProjectValueChange}
                  itemToStringValue={(workspaceId) =>
                    projectWorkspaces.find((workspace) => workspace.id === workspaceId)?.name ??
                    workspaceId
                  }
                >
                  <ComboboxInput
                    aria-label="Project"
                    placeholder="Select project"
                    className="w-48 shrink-0"
                    disabled={workspaceActionsDisabled}
                  />
                  <ComboboxContent>
                    <ComboboxEmpty>No matching projects</ComboboxEmpty>
                    <ComboboxList>
                      {(workspaceId) => {
                        const workspace = projectWorkspaces.find((item) => item.id === workspaceId);
                        if (!workspace) {
                          return null;
                        }

                        return (
                          <ComboboxItem key={workspace.id} value={workspace.id}>
                            <span className="min-w-0 truncate">{workspace.name}</span>
                          </ComboboxItem>
                        );
                      }}
                    </ComboboxList>
                  </ComboboxContent>
                </Combobox>
              ) : null}
              <TabsList className="shrink-0">
                <TabsTrigger value="agent">Agent</TabsTrigger>
                <TabsTrigger value="project">Project</TabsTrigger>
              </TabsList>
            </div>

            {overviewError ? (
              <div className="shrink-0 border-t border-input">
                <ErrorNotice
                  title="Workspace refresh failed"
                  error={overviewError}
                  onRetry={() => void handleRefresh()}
                  isRetrying={isRefreshing}
                />
              </div>
            ) : null}

            <TabsContent value="agent" className="flex min-h-0 min-w-0 flex-1 flex-col">
              {filteredHarnesses.length > 0 ? (
                <ScrollArea className="min-h-0 min-w-0 flex-1" aria-busy={isRefreshing}>
                  <WorkspaceList items={filteredHarnesses.map(harnessToListEntry)} />
                </ScrollArea>
              ) : overview.harnesses.length === 0 ? (
                <EmptyState
                  title="No agents available"
                  description="The backend did not report any agent harnesses."
                />
              ) : (
                <EmptyState
                  title="No matching results"
                  description="Try a different search term."
                />
              )}
            </TabsContent>

            <TabsContent value="project" className="flex min-h-0 min-w-0 flex-1 flex-col">
              {filteredProjects.length > 0 ? (
                <ScrollArea className="min-h-0 min-w-0 flex-1" aria-busy={workspaceActionsDisabled}>
                  <WorkspaceList
                    items={filteredProjects.map(projectToListEntry)}
                    selectedId={selectedProjectId}
                    disabled={workspaceActionsDisabled}
                    onSelect={handleSelectProject}
                    onClearSelection={clearSelectedProject}
                  />
                </ScrollArea>
              ) : projectWorkspaces.length === 0 ? (
                <EmptyState
                  title="No projects yet"
                  description="Use Add project to register a project directory."
                />
              ) : (
                <EmptyState
                  title="No matching results"
                  description="Try a different search term."
                />
              )}
            </TabsContent>
          </Tabs>
        )}
      </div>

      <Dialog open={isProjectListOpen} onOpenChange={setIsProjectListOpen}>
        <DialogContent className="flex h-[min(80vh,720px)] max-w-2xl flex-col overflow-hidden">
          <DialogHeader>
            <DialogTitle>Project list</DialogTitle>
          </DialogHeader>
          <ScrollArea className="min-h-0 flex-1 border border-input">
            {filteredProjects.length > 0 ? (
              <SelectionList
                items={filteredProjects.map((project) => ({
                  id: project.id,
                  name: project.name,
                  subtitle: project.kind.root.display,
                }))}
                selectedId={selectedProjectId}
                disabled={workspaceActionsDisabled}
                onToggle={(workspaceId, checked) => {
                  if (checked) {
                    void handleSelectProject(workspaceId);
                  } else if (workspaceId === selectedProjectId) {
                    clearSelectedProject();
                  }
                }}
              />
            ) : projectWorkspaces.length === 0 ? (
              <EmptyState
                title="No projects yet"
                description="Use Add project to register a project directory."
              />
            ) : (
              <EmptyState title="No matching results" description="Try a different search term." />
            )}
          </ScrollArea>
          <div className="flex shrink-0 items-center justify-between gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-label="Select all unavailable"
              title="Bulk project selection is unavailable"
              disabled
            >
              <RiCheckboxMultipleLine aria-hidden="true" data-icon="inline-start" />
              Select all
            </Button>
            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                aria-busy={isSelectingProject || isMutating}
                onClick={() => void handleAddProject()}
                disabled={isSelectingProject || isMutating}
              >
                <RiAddLine aria-hidden="true" data-icon="inline-start" />
                Add
              </Button>
              <Button
                type="button"
                variant="destructive"
                size="sm"
                onClick={() => void handleSync()}
                disabled={!selectedProject || isMutating || isRefreshing || isObserving}
                title="Reconcile the selected workspace; this operation may write changes"
              >
                <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                {isSyncingSelected ? "Syncing…" : "Sync"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
