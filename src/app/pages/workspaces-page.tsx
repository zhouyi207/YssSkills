import { useMemo, useState, type FormEvent } from "react";
import {
  RiAddLine,
  RiCheckboxMultipleLine,
  RiDeleteBinLine,
  RiEditLine,
  RiFolderLine,
  RiFolderOpenLine,
  RiRefreshLine,
  RiSearchLine,
} from "@remixicon/react";
import { toast } from "sonner";

import { useCatalogSkills } from "@/app/hooks/use-catalog-skills";
import { useWorkspaces } from "@/app/hooks/use-workspaces";
import { selectDirectory } from "@/app/services/directory-picker";
import { countWorkspaceReconcileIssues } from "@/app/services/workspaces-service";
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
  useComboboxAnchor,
} from "@/components/ui/combobox";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import type { IpcError } from "@/shared/types/ipc";
import type { CatalogSkillSummaryDto } from "@/shared/types/skills";
import type {
  DetectAgentsResponseDto,
  DetectedAgentDto,
  HarnessSummaryDto,
  ProjectAgentDto,
  WorkspaceSummaryDto,
} from "@/shared/types/workspaces";

type AgentSkillTab = "item" | "set";
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

function pathIsWithinRoot(path: string, root: string) {
  const normalize = (value: string) => {
    const normalized = value.replace(/\\/g, "/").replace(/\/+$/, "") || "/";
    const windowsPath = /^[a-z]:/i.test(normalized) || normalized.startsWith("//");
    return windowsPath ? normalized.toLowerCase() : normalized;
  };
  const normalizedPath = normalize(path);
  const normalizedRoot = normalize(root);
  return normalizedPath === normalizedRoot || normalizedPath.startsWith(`${normalizedRoot}/`);
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
  selectedIds,
  disabled = false,
  onSelect,
  onToggleSelection,
  onClearSelection,
  onEdit,
}: {
  items: ReadonlyArray<WorkspaceListEntry>;
  selectedId?: string | null;
  selectedIds?: ReadonlySet<string>;
  disabled?: boolean;
  onSelect?: (id: string) => void | Promise<void>;
  onToggleSelection?: (id: string, checked?: boolean) => void;
  onClearSelection?: () => void;
  onEdit?: (id: string) => void;
}) {
  const keepCheckboxesVisible = selectedId !== null || (selectedIds?.size ?? 0) > 0;

  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {items.map((entry) => {
        const isSelected = selectedIds?.has(entry.id) ?? selectedId === entry.id;
        const isSelectable = onSelect !== undefined || onToggleSelection !== undefined;
        const activate = () => {
          if (onToggleSelection) {
            onToggleSelection(entry.id);
          } else if (onSelect) {
            void onSelect(entry.id);
          }
        };

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
                activate();
              }
            }}
            onKeyDown={(event) => {
              if (!isSelectable || disabled || event.target !== event.currentTarget) {
                return;
              }

              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                activate();
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
                  if (onToggleSelection) {
                    onToggleSelection(entry.id, checked === true);
                    return;
                  }
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
              aria-label={onEdit ? `Edit ${entry.name}` : `Edit unavailable for ${entry.name}`}
              title={onEdit ? `Edit ${entry.name}` : "Workspace editing is unavailable"}
              className="justify-self-end"
              disabled={disabled || !onEdit}
              onClick={(event) => {
                event.stopPropagation();
                if (!disabled) {
                  onEdit?.(entry.id);
                }
              }}
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

function AgentSkillList({
  skills,
  selectedSkillIds,
  disabled,
  onToggle,
}: {
  skills: ReadonlyArray<CatalogSkillSummaryDto>;
  selectedSkillIds: ReadonlySet<string>;
  disabled: boolean;
  onToggle?: (skillId: string, checked: boolean) => void;
}) {
  return (
    <div role="list" className="flex flex-col p-1">
      {skills.map((skill) => (
        <label
          key={skill.id}
          className={cn(
            "flex min-w-0 items-center gap-2 px-2 py-1.5 hover:bg-muted",
            disabled ? "cursor-default" : "cursor-pointer",
          )}
        >
          <Checkbox
            aria-label={`${skill.name} assignment`}
            checked={selectedSkillIds.has(skill.id)}
            disabled={disabled}
            onCheckedChange={(checked) => onToggle?.(skill.id, checked === true)}
          />
          <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
            <span className="min-w-0 flex-1 truncate text-xs font-medium">{skill.name}</span>
            <span className="shrink-0 truncate text-[0.65rem] text-muted-foreground">
              {skill.version ? `v${skill.version}` : skill.source.kind}
            </span>
          </div>
        </label>
      ))}
    </div>
  );
}

function DetectedAgentList({
  agents,
  selectedDetectorIds,
  disabled,
  onToggle,
}: {
  agents: ReadonlyArray<DetectedAgentDto>;
  selectedDetectorIds: ReadonlySet<string>;
  disabled: boolean;
  onToggle: (detectorId: string, checked: boolean) => void;
}) {
  return (
    <div role="list" className="flex flex-col p-1">
      {agents.map((agent) => (
        <label
          key={agent.detectorId}
          className={cn(
            "flex min-w-0 items-center gap-2 px-2 py-1.5 hover:bg-muted",
            agent.configured || disabled ? "cursor-default" : "cursor-pointer",
          )}
        >
          <Checkbox
            aria-label={`Select ${agent.displayName}`}
            checked={agent.configured || selectedDetectorIds.has(agent.detectorId)}
            disabled={agent.configured || disabled}
            onCheckedChange={(checked) => onToggle(agent.detectorId, checked === true)}
          />
          <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
            <span className="min-w-0 flex-1 truncate text-xs font-medium">{agent.displayName}</span>
            <span
              className="max-w-[65%] shrink-0 truncate text-[0.65rem] text-muted-foreground"
              title={agent.agentRoot.display}
            >
              {agent.configured ? "Added" : `${agent.skillCount} skills`} ·{" "}
              {agent.agentRoot.display}
            </span>
          </div>
        </label>
      ))}
    </div>
  );
}

function ProjectAgentDetectionList({ agents }: { agents: ReadonlyArray<ProjectAgentDto> }) {
  return (
    <div role="list" className="flex flex-col p-1">
      {agents.map((agent) => (
        <div
          key={agent.id}
          role="listitem"
          className="flex min-w-0 items-center gap-2 px-2 py-1.5 hover:bg-muted"
        >
          <Checkbox aria-label={`${agent.displayName} detected`} checked disabled />
          <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
            <span className="min-w-0 flex-1 truncate text-xs font-medium">{agent.displayName}</span>
            <span
              className="max-w-[65%] shrink-0 truncate text-[0.65rem] text-muted-foreground"
              title={agent.path.display}
            >
              {agent.error ? agent.error.message : `${agent.skillCount} skills`} ·{" "}
              {agent.path.display}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

function harnessToListEntry(harness: HarnessSummaryDto): WorkspaceListEntry {
  const path =
    harness.probe?.agentPath.display ??
    (harness.error
      ? `${harness.error.message} (${harness.error.code})`
      : "Unavailable (probe unavailable)");

  return {
    id: harness.id,
    name: harness.displayName,
    path,
    count: harness.skillCount,
  };
}

function projectAgentToListEntry(agent: ProjectAgentDto): WorkspaceListEntry {
  return {
    id: agent.id,
    name: agent.displayName,
    path: agent.error ? `${agent.path.display} · ${agent.error.message}` : agent.path.display,
    count: agent.skillCount,
  };
}

export function WorkspacesPage() {
  const projectComboboxAnchor = useComboboxAnchor();
  const {
    skills: catalogSkills,
    error: catalogSkillsError,
    isLoading: isCatalogSkillsLoading,
  } = useCatalogSkills();
  const {
    overview,
    error: overviewError,
    isLoading,
    isRefreshing,
    refresh,
    observation,
    observationError,
    isObserving,
    mutationError,
    isMutating,
    detectionError,
    isDetectingAgents,
    observe,
    createProject,
    detectAgents,
    addDetectedAgents,
    deleteAgents,
    copyProjectAgentSkills,
    deleteProjectAgents,
    saveAgent,
    reconcile,
    clearMutationError,
  } = useWorkspaces();
  const [query, setQuery] = useState("");
  const [activeTab, setActiveTab] = useState<WorkspaceTab>("agent");
  const [selectedAgentIds, setSelectedAgentIds] = useState<Set<string>>(() => new Set());
  const [selectedProjectAgentIds, setSelectedProjectAgentIds] = useState<Set<string>>(
    () => new Set(),
  );
  const [isAutoDetectOpen, setIsAutoDetectOpen] = useState(false);
  const [agentDetection, setAgentDetection] = useState<DetectAgentsResponseDto | null>(null);
  const [selectedDetectorIds, setSelectedDetectorIds] = useState<Set<string>>(() => new Set());
  const [isDeleteAgentsOpen, setIsDeleteAgentsOpen] = useState(false);
  const [isProjectAutoDetectOpen, setIsProjectAutoDetectOpen] = useState(false);
  const [projectDetection, setProjectDetection] = useState<ProjectAgentDto[] | null>(null);
  const [isDeleteProjectAgentsOpen, setIsDeleteProjectAgentsOpen] = useState(false);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [isProjectListOpen, setIsProjectListOpen] = useState(false);
  const [isAddProjectOpen, setIsAddProjectOpen] = useState(false);
  const [newProjectName, setNewProjectName] = useState("");
  const [newProjectPath, setNewProjectPath] = useState("");
  const [isSelectingProjectPath, setIsSelectingProjectPath] = useState(false);
  const [isAddAgentOpen, setIsAddAgentOpen] = useState(false);
  const [isAddProjectAgentOpen, setIsAddProjectAgentOpen] = useState(false);
  const [newAgentName, setNewAgentName] = useState("");
  const [newAgentPath, setNewAgentPath] = useState("");
  const [newAgentSkillIds, setNewAgentSkillIds] = useState<Set<string>>(() => new Set());
  const [isSelectingAgentPath, setIsSelectingAgentPath] = useState(false);
  const [editingAgentId, setEditingAgentId] = useState<string | null>(null);
  const [editingAgentSkillTab, setEditingAgentSkillTab] = useState<AgentSkillTab>("item");
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
          harness.probe?.agentPath.display ?? "unavailable",
          harness.probe?.detectionStatus ?? "unavailable",
          harness.skillCount,
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
  const visibleAgentIds = filteredHarnesses.map((harness) => harness.id);
  const allVisibleAgentsSelected =
    visibleAgentIds.length > 0 && visibleAgentIds.every((id) => selectedAgentIds.has(id));
  const selectedProject = projectWorkspaces.find((workspace) => workspace.id === selectedProjectId);
  const selectedProjectObservation =
    observation?.workspace.id === selectedProjectId ? observation : null;
  const filteredProjectAgents = useMemo(
    () =>
      (selectedProjectObservation?.projectAgents ?? []).filter((agent) =>
        matchesQuery(normalizedQuery, [
          agent.displayName,
          agent.path.display,
          agent.skillCount,
          agent.error?.message ?? "",
        ]),
      ),
    [normalizedQuery, selectedProjectObservation],
  );
  const visibleProjectAgentIds = filteredProjectAgents.map((agent) => agent.id);
  const allVisibleProjectAgentsSelected =
    visibleProjectAgentIds.length > 0 &&
    visibleProjectAgentIds.every((id) => selectedProjectAgentIds.has(id));
  const editingAgent = overview?.harnesses.find((harness) => harness.id === editingAgentId) ?? null;

  const handleTabChange = (value: string) => {
    if (value === "agent" || value === "project") {
      setActiveTab(value);
    }
  };

  const clearSelectedProject = () => {
    setSelectedProjectId(null);
    setSelectedProjectAgentIds(new Set());
  };

  const toggleAgentSelection = (agentId: string, checked?: boolean) => {
    setSelectedAgentIds((current) => {
      const next = new Set(current);
      const shouldSelect = checked ?? !next.has(agentId);
      if (shouldSelect) {
        next.add(agentId);
      } else {
        next.delete(agentId);
      }
      return next;
    });
  };

  const toggleSelectAllAgents = () => {
    setSelectedAgentIds((current) => {
      const next = new Set(current);
      visibleAgentIds.forEach((agentId) => {
        if (allVisibleAgentsSelected) {
          next.delete(agentId);
        } else {
          next.add(agentId);
        }
      });
      return next;
    });
  };

  const toggleProjectAgentSelection = (agentId: string, checked?: boolean) => {
    setSelectedProjectAgentIds((current) => {
      const next = new Set(current);
      const shouldSelect = checked ?? !next.has(agentId);
      if (shouldSelect) {
        next.add(agentId);
      } else {
        next.delete(agentId);
      }
      return next;
    });
  };

  const toggleSelectAllProjectAgents = () => {
    setSelectedProjectAgentIds((current) => {
      const next = new Set(current);
      visibleProjectAgentIds.forEach((agentId) => {
        if (allVisibleProjectAgentsSelected) {
          next.delete(agentId);
        } else {
          next.add(agentId);
        }
      });
      return next;
    });
  };

  const openAutoDetectDialog = async () => {
    clearMutationError();
    setAgentDetection(null);
    setSelectedDetectorIds(new Set());
    setIsAutoDetectOpen(true);
    const outcome = await detectAgents();
    if (outcome) {
      setAgentDetection(outcome);
    }
  };

  const toggleDetectedAgent = (detectorId: string, checked: boolean) => {
    setSelectedDetectorIds((current) => {
      const next = new Set(current);
      if (checked) {
        next.add(detectorId);
      } else {
        next.delete(detectorId);
      }
      return next;
    });
  };

  const handleAddDetectedAgents = async () => {
    const outcome = await addDetectedAgents(Array.from(selectedDetectorIds));
    if (!outcome) {
      toast.error("Unable to add the detected Agents.");
      return;
    }
    setIsAutoDetectOpen(false);
    setSelectedDetectorIds(new Set());
    toast.success(
      `Added ${outcome.addedAgentIds.length} Agent${outcome.addedAgentIds.length === 1 ? "" : "s"}.`,
    );
  };

  const handleDeleteAgents = async () => {
    const outcome = await deleteAgents(Array.from(selectedAgentIds));
    if (!outcome) {
      toast.error("Unable to delete the selected Agents.");
      return;
    }
    setSelectedAgentIds(new Set());
    setIsDeleteAgentsOpen(false);
    toast.success(
      `Deleted ${outcome.deletedAgentIds.length} Agent${outcome.deletedAgentIds.length === 1 ? "" : "s"} and ${outcome.deletedSkillCount} Skill entr${outcome.deletedSkillCount === 1 ? "y" : "ies"}.`,
    );
  };

  const handleDeleteProjectAgents = async () => {
    if (!selectedProjectId || selectedProjectAgentIds.size === 0) {
      return;
    }
    const outcome = await deleteProjectAgents(
      selectedProjectId,
      Array.from(selectedProjectAgentIds),
    );
    if (!outcome) {
      toast.error("Unable to delete the selected Project Agents.");
      return;
    }
    setSelectedProjectAgentIds(new Set());
    setIsDeleteProjectAgentsOpen(false);
    toast.success(
      `Deleted ${outcome.deletedAgentIds.length} Project Agent${outcome.deletedAgentIds.length === 1 ? "" : "s"} and ${outcome.deletedSkillCount} Skill entr${outcome.deletedSkillCount === 1 ? "y" : "ies"}.`,
    );
  };

  const openAddAgentDialog = () => {
    clearMutationError();
    setEditingAgentId(null);
    setNewAgentName("");
    setNewAgentPath("");
    setNewAgentSkillIds(new Set());
    setEditingAgentSkillTab("item");
    setIsAddProjectOpen(false);
    setIsAddProjectAgentOpen(false);
    setIsProjectListOpen(false);
    setIsAddAgentOpen(true);
  };

  const openEditAgentDialog = (agentId: string) => {
    const agent = overview?.harnesses.find((harness) => harness.id === agentId);
    if (!agent || !overview) {
      return;
    }
    clearMutationError();
    setIsAddAgentOpen(false);
    setIsAddProjectAgentOpen(false);
    setEditingAgentId(agentId);
    setNewAgentName(agent.displayName);
    setNewAgentPath(agent.probe?.agentPath.value ?? "");
    setNewAgentSkillIds(new Set(agent.linkedSkillIds));
    setEditingAgentSkillTab("item");
  };

  const openAddProjectAgentDialog = () => {
    if (!selectedProjectId || !selectedProject?.kind.root.value) {
      return;
    }
    clearMutationError();
    setEditingAgentId(null);
    setNewAgentName("");
    setNewAgentPath("");
    setNewAgentSkillIds(new Set());
    setEditingAgentSkillTab("item");
    setIsAddAgentOpen(false);
    setIsAddProjectOpen(false);
    setIsProjectListOpen(false);
    setIsAddProjectAgentOpen(true);
  };

  const openProjectAutoDetectDialog = async () => {
    if (!selectedProjectId) {
      return;
    }
    setProjectDetection(null);
    setIsProjectAutoDetectOpen(true);
    const outcome = await observe(selectedProjectId);
    if (outcome) {
      setProjectDetection(outcome.projectAgents);
    }
  };

  const handleSelectAgentPath = async () => {
    setIsSelectingAgentPath(true);
    try {
      const projectRoot = isAddProjectAgentOpen
        ? (selectedProject?.kind.root.value ?? undefined)
        : undefined;
      if (isAddProjectAgentOpen && !projectRoot) {
        toast.error("The selected Project path cannot be represented safely.");
        return;
      }
      const selectedPath = await selectDirectory(
        isAddProjectAgentOpen ? "Select Project Agent directory" : "Select agent directory",
        projectRoot,
      );
      if (selectedPath) {
        if (projectRoot && !pathIsWithinRoot(selectedPath, projectRoot)) {
          toast.error("The Agent path must be inside the selected Project directory.");
          return;
        }
        setNewAgentPath(selectedPath);
      }
    } catch {
      toast.error("Unable to open the agent path picker.");
    } finally {
      setIsSelectingAgentPath(false);
    }
  };

  const toggleNewAgentSkill = (skillId: string, checked: boolean) => {
    setNewAgentSkillIds((current) => {
      const next = new Set(current);
      if (checked) {
        next.add(skillId);
      } else {
        next.delete(skillId);
      }
      return next;
    });
  };

  const handleSaveAgent = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const agentRoot = newAgentPath.trim();
    if (!agentRoot) {
      toast.error("Choose an accessible Agent directory.");
      return;
    }
    if (!newAgentName.trim()) {
      toast.error("Enter an Agent name.");
      return;
    }
    if ((isAddAgentOpen || isAddProjectAgentOpen) && newAgentSkillIds.size === 0) {
      toast.error("Select at least one Skill.");
      return;
    }

    if (isAddProjectAgentOpen) {
      if (!selectedProjectId) {
        toast.error("Select a Project first.");
        return;
      }
      const copied = await copyProjectAgentSkills({
        workspaceId: selectedProjectId,
        agentRoot,
        skillIds: Array.from(newAgentSkillIds),
      });
      if (!copied) {
        toast.error("Unable to copy Skills into the Project Agent.");
        return;
      }
      setIsAddProjectAgentOpen(false);
      toast.success(
        `Copied ${copied.copiedSkillIds.length} Skill${copied.copiedSkillIds.length === 1 ? "" : "s"} into the Project Agent.`,
      );
      return;
    }

    const outcome = await saveAgent({
      agentId: isAddAgentOpen ? null : editingAgentId,
      displayName: newAgentName.trim(),
      agentRoot,
      skillIds: Array.from(newAgentSkillIds),
    });
    if (!outcome) {
      toast.error("Unable to save the Agent Skills.");
      return;
    }

    setIsAddAgentOpen(false);
    setEditingAgentId(null);
    toast.success(
      `Saved ${outcome.linkedSkillIds.length} linked Skill${outcome.linkedSkillIds.length === 1 ? "" : "s"}${outcome.removedSkillIds.length > 0 ? `; removed ${outcome.removedSkillIds.length} old link${outcome.removedSkillIds.length === 1 ? "" : "s"}` : ""}.`,
    );
  };

  const handleSelectProject = async (workspaceId: string) => {
    setSelectedProjectId(workspaceId);
    setSelectedProjectAgentIds(new Set());
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

  const openAddProjectDialog = () => {
    setIsAddAgentOpen(false);
    setIsAddProjectAgentOpen(false);
    setEditingAgentId(null);
    setNewProjectName("");
    setNewProjectPath("");
    setIsProjectListOpen(false);
    setIsAddProjectOpen(true);
  };

  const handleSelectProjectPath = async () => {
    setIsSelectingProjectPath(true);

    try {
      const root = await selectDirectory("Select project directory");

      if (!root) {
        return;
      }

      setNewProjectPath(root);
      setNewProjectName((current) => (current.trim() ? current : getDirectoryName(root)));
    } catch {
      toast.error("Unable to open the project folder picker.");
    } finally {
      setIsSelectingProjectPath(false);
    }
  };

  const handleAddProject = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const name = newProjectName.trim();
    const root = newProjectPath.trim();
    if (!name || !root) {
      toast.error("Enter a project name and choose its directory.");
      return;
    }

    const created = await createProject(name, root);
    if (!created) {
      toast.error("Unable to add the project. See the error details on this page.");
      return;
    }

    setActiveTab("project");
    setQuery("");
    setSelectedProjectId(created.id);
    setIsAddProjectOpen(false);
    setIsProjectListOpen(true);
    await observe(created.id);
    toast.success("Project added.");
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
    const currentAgentIds = new Set(nextOverview.harnesses.map((harness) => harness.id));
    setSelectedAgentIds(
      (current) => new Set(Array.from(current).filter((agentId) => currentAgentIds.has(agentId))),
    );

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

      const issueCount = countWorkspaceReconcileIssues(outcome);
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
            aria-pressed={
              activeTab === "agent" ? allVisibleAgentsSelected : allVisibleProjectAgentsSelected
            }
            disabled={
              isMutating ||
              isRefreshing ||
              (activeTab === "agent"
                ? visibleAgentIds.length === 0
                : !selectedProjectId || visibleProjectAgentIds.length === 0)
            }
            onClick={activeTab === "agent" ? toggleSelectAllAgents : toggleSelectAllProjectAgents}
          >
            <RiCheckboxMultipleLine aria-hidden="true" data-icon="inline-start" />
            {(activeTab === "agent" ? allVisibleAgentsSelected : allVisibleProjectAgentsSelected)
              ? "Deselect all"
              : "Select all"}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-haspopup="dialog"
            aria-label={activeTab === "agent" ? "Add agent" : "Add Agent to selected Project"}
            title={activeTab === "agent" ? "Add an Agent" : "Add an Agent to the selected Project"}
            onClick={activeTab === "agent" ? openAddAgentDialog : openAddProjectAgentDialog}
            disabled={
              isMutating ||
              isSelectingAgentPath ||
              (activeTab === "project" && (!selectedProjectId || !selectedProject?.kind.root.value))
            }
          >
            <RiAddLine aria-hidden="true" data-icon="inline-start" />
            Add
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-haspopup="dialog"
            aria-busy={activeTab === "agent" ? isDetectingAgents : isObserving}
            title={
              activeTab === "agent"
                ? "Detect Agents using built-in Harness adapters"
                : "Detect Agents inside the selected Project"
            }
            onClick={() =>
              void (activeTab === "agent" ? openAutoDetectDialog() : openProjectAutoDetectDialog())
            }
            disabled={
              isDetectingAgents ||
              isObserving ||
              isMutating ||
              isRefreshing ||
              (activeTab === "project" && !selectedProjectId)
            }
          >
            <RiSearchLine aria-hidden="true" data-icon="inline-start" />
            Auto detect
          </Button>
          <Button
            type="button"
            variant="destructive"
            size="sm"
            aria-label={
              activeTab === "agent" ? "Delete selected Agents" : "Delete selected Project Agents"
            }
            onClick={() => {
              clearMutationError();
              if (activeTab === "agent") {
                setIsDeleteAgentsOpen(true);
              } else {
                setIsDeleteProjectAgentsOpen(true);
              }
            }}
            disabled={
              isMutating ||
              (activeTab === "agent"
                ? selectedAgentIds.size === 0
                : !selectedProjectId || selectedProjectAgentIds.size === 0)
            }
          >
            <RiDeleteBinLine aria-hidden="true" data-icon="inline-start" />
            Delete
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
            title="Rescan Agent skills from HOME and reload Project workspace data"
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
                <div ref={projectComboboxAnchor} className="min-w-0 w-1/2 shrink-0">
                  <Combobox
                    items={projectWorkspaces.map((workspace) => workspace.id)}
                    value={selectedProjectId ?? ""}
                    onValueChange={handleProjectValueChange}
                    itemToStringLabel={(workspaceId) =>
                      projectWorkspaces.find((workspace) => workspace.id === workspaceId)?.name ??
                      workspaceId
                    }
                  >
                    <ComboboxInput
                      aria-label="Project"
                      placeholder="Select project"
                      className="w-full"
                      disabled={workspaceActionsDisabled}
                    />
                    <ComboboxContent
                      anchor={projectComboboxAnchor}
                      className="w-(--anchor-width) min-w-(--anchor-width) max-w-(--anchor-width)"
                    >
                      <ComboboxEmpty>No matching projects</ComboboxEmpty>
                      <ComboboxList>
                        {(workspaceId) => {
                          const workspace = projectWorkspaces.find(
                            (item) => item.id === workspaceId,
                          );
                          if (!workspace) {
                            return null;
                          }

                          return (
                            <ComboboxItem key={workspace.id} value={workspace.id}>
                              <div className="flex min-w-0 flex-1 items-center justify-between gap-3">
                                <span className="min-w-0 flex-1 truncate font-medium">
                                  {workspace.name}
                                </span>
                                <span
                                  className="max-w-[65%] shrink-0 truncate text-[0.65rem] text-muted-foreground"
                                  title={workspace.kind.root.display}
                                >
                                  {workspace.kind.root.display}
                                </span>
                              </div>
                            </ComboboxItem>
                          );
                        }}
                      </ComboboxList>
                    </ComboboxContent>
                  </Combobox>
                </div>
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
                  <WorkspaceList
                    items={filteredHarnesses.map(harnessToListEntry)}
                    selectedIds={selectedAgentIds}
                    disabled={isRefreshing}
                    onToggleSelection={toggleAgentSelection}
                    onEdit={openEditAgentDialog}
                  />
                </ScrollArea>
              ) : overview.harnesses.length === 0 ? (
                <EmptyState
                  title="No agents available"
                  description="No agent skills directories were found in your user profile."
                />
              ) : (
                <EmptyState
                  title="No matching results"
                  description="Try a different search term."
                />
              )}
            </TabsContent>

            <TabsContent value="project" className="flex min-h-0 min-w-0 flex-1 flex-col">
              {!selectedProjectId ? (
                <EmptyState
                  title="Select a project"
                  description="Choose a project to inspect its Agent directories."
                />
              ) : isObserving && !selectedProjectObservation ? (
                <EmptyState
                  title="Inspecting project…"
                  description="Checking the selected project directory for Agents."
                />
              ) : observationError && !selectedProjectObservation ? (
                <ErrorNotice
                  title="Unable to inspect project"
                  error={observationError}
                  onRetry={() => void observe(selectedProjectId)}
                  isRetrying={isObserving}
                />
              ) : filteredProjectAgents.length > 0 ? (
                <ScrollArea className="min-h-0 min-w-0 flex-1" aria-busy={workspaceActionsDisabled}>
                  <WorkspaceList
                    items={filteredProjectAgents.map(projectAgentToListEntry)}
                    selectedIds={selectedProjectAgentIds}
                    disabled={workspaceActionsDisabled}
                    onToggleSelection={toggleProjectAgentSelection}
                  />
                </ScrollArea>
              ) : selectedProjectObservation?.projectAgents.length === 0 ? (
                <EmptyState
                  title="No Agents found"
                  description="The selected project does not contain a recognized Agent directory."
                />
              ) : selectedProjectObservation ? (
                <EmptyState
                  title="No matching results"
                  description="Try a different search term."
                />
              ) : (
                <EmptyState
                  title="Project observation unavailable"
                  description="Select the project again to inspect its directory."
                />
              )}
            </TabsContent>
          </Tabs>
        )}
      </div>

      <Dialog
        open={isAutoDetectOpen}
        onOpenChange={(open) => {
          if (!isMutating && !isDetectingAgents) {
            setIsAutoDetectOpen(open);
          }
        }}
      >
        <DialogContent className="flex h-[min(80vh,720px)] max-w-2xl flex-col overflow-hidden">
          <DialogHeader>
            <DialogTitle>Auto detect agents</DialogTitle>
          </DialogHeader>

          <ScrollArea className="min-h-0 flex-1 border border-input" aria-busy={isDetectingAgents}>
            {agentDetection?.agents.length ? (
              <DetectedAgentList
                agents={agentDetection.agents}
                selectedDetectorIds={selectedDetectorIds}
                disabled={isMutating}
                onToggle={toggleDetectedAgent}
              />
            ) : isDetectingAgents ? (
              <EmptyState
                title="Detecting Agents…"
                description="Checking built-in Harness adapter locations."
              />
            ) : detectionError ? (
              <ErrorNotice title="Unable to detect Agents" error={detectionError} />
            ) : (
              <EmptyState
                title="No Agents detected"
                description="No built-in Harness adapter reported an existing Agent directory."
              />
            )}

            {agentDetection?.diagnostics.length ? (
              <div role="alert" className="m-1 border border-destructive/40 px-3 py-3">
                <p className="font-medium">
                  {agentDetection.diagnostics.length} detection diagnostic
                  {agentDetection.diagnostics.length === 1 ? "" : "s"}
                </p>
                <div className="mt-2 space-y-2">
                  {agentDetection.diagnostics.map((diagnostic) => (
                    <div key={diagnostic.detectorId}>
                      <p className="font-medium">{diagnostic.displayName}</p>
                      <p className="text-muted-foreground">
                        {diagnostic.error.message}{" "}
                        <span className="font-mono text-[0.65rem]">({diagnostic.error.code})</span>
                      </p>
                    </div>
                  ))}
                </div>
              </div>
            ) : null}
          </ScrollArea>

          {mutationError ? (
            <div role="alert" className="shrink-0 border border-destructive/40 px-3 py-2">
              <p className="font-medium">Unable to add detected Agents</p>
              <p className="text-muted-foreground">
                {mutationError.message}{" "}
                <span className="font-mono text-[0.65rem]">({mutationError.code})</span>
              </p>
            </div>
          ) : null}

          <DialogFooter className="shrink-0">
            <DialogClose asChild>
              <Button type="button" variant="outline" disabled={isMutating || isDetectingAgents}>
                Cancel
              </Button>
            </DialogClose>
            <Button
              type="button"
              aria-busy={isMutating}
              disabled={selectedDetectorIds.size === 0 || isMutating}
              onClick={() => void handleAddDetectedAgents()}
            >
              {isMutating
                ? "Adding…"
                : `Add ${selectedDetectorIds.size} Agent${selectedDetectorIds.size === 1 ? "" : "s"}`}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={isProjectAutoDetectOpen}
        onOpenChange={(open) => {
          if (!isObserving) {
            setIsProjectAutoDetectOpen(open);
          }
        }}
      >
        <DialogContent className="flex h-[min(80vh,720px)] max-w-2xl flex-col overflow-hidden">
          <DialogHeader>
            <DialogTitle>Detect Project agents</DialogTitle>
          </DialogHeader>
          <ScrollArea className="min-h-0 flex-1 border border-input" aria-busy={isObserving}>
            {projectDetection?.length ? (
              <ProjectAgentDetectionList agents={projectDetection} />
            ) : isObserving ? (
              <EmptyState
                title="Inspecting project…"
                description="Checking the selected Project for Agent directories."
              />
            ) : observationError ? (
              <ErrorNotice title="Unable to inspect Project" error={observationError} />
            ) : (
              <EmptyState
                title="No Agents found"
                description="The selected Project does not contain a recognized Agent directory."
              />
            )}
          </ScrollArea>
          <DialogFooter className="shrink-0">
            <DialogClose asChild>
              <Button type="button">Close</Button>
            </DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={isDeleteAgentsOpen}
        onOpenChange={(open) => {
          if (!isMutating) {
            setIsDeleteAgentsOpen(open);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete selected Agents?</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">
            This removes {selectedAgentIds.size} Agent configuration
            {selectedAgentIds.size === 1 ? "" : "s"} and deletes every Skill entry in each
            corresponding skills directory. Skill links are removed without deleting their targets.
          </p>
          {mutationError ? (
            <div role="alert" className="border border-destructive/40 px-3 py-2">
              <p className="font-medium">Unable to delete selected Agents</p>
              <p className="text-muted-foreground">
                {mutationError.message}{" "}
                <span className="font-mono text-[0.65rem]">({mutationError.code})</span>
              </p>
            </div>
          ) : null}
          <DialogFooter>
            <DialogClose asChild>
              <Button type="button" variant="outline" disabled={isMutating}>
                Cancel
              </Button>
            </DialogClose>
            <Button
              type="button"
              variant="destructive"
              aria-busy={isMutating}
              disabled={isMutating || selectedAgentIds.size === 0}
              onClick={() => void handleDeleteAgents()}
            >
              {isMutating ? "Deleting…" : "Delete Agents and Skills"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={isDeleteProjectAgentsOpen}
        onOpenChange={(open) => {
          if (!isMutating) {
            setIsDeleteProjectAgentsOpen(open);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete selected Project Agents?</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">
            This deletes all Skills in {selectedProjectAgentIds.size} selected Agent director
            {selectedProjectAgentIds.size === 1 ? "y" : "ies"} inside
            {` ${selectedProject?.name ?? "the selected Project"}`}. The Project Workspace and
            non-Skill files are preserved.
          </p>
          {mutationError ? (
            <div role="alert" className="border border-destructive/40 px-3 py-2">
              <p className="font-medium">Unable to delete selected Project Agents</p>
              <p className="text-muted-foreground">
                {mutationError.message}{" "}
                <span className="font-mono text-[0.65rem]">({mutationError.code})</span>
              </p>
            </div>
          ) : null}
          <DialogFooter>
            <DialogClose asChild>
              <Button type="button" variant="outline" disabled={isMutating}>
                Cancel
              </Button>
            </DialogClose>
            <Button
              type="button"
              variant="destructive"
              aria-busy={isMutating}
              disabled={isMutating || !selectedProjectId || selectedProjectAgentIds.size === 0}
              onClick={() => void handleDeleteProjectAgents()}
            >
              {isMutating ? "Deleting…" : "Delete Agents and Skills"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={isAddAgentOpen || isAddProjectAgentOpen || editingAgent !== null}
        onOpenChange={(open) => {
          if (!open) {
            setIsAddAgentOpen(false);
            setIsAddProjectAgentOpen(false);
            setEditingAgentId(null);
          }
        }}
      >
        <DialogContent className="flex h-[min(80vh,720px)] max-w-2xl flex-col overflow-hidden">
          {isAddAgentOpen || isAddProjectAgentOpen || editingAgent ? (
            <>
              <DialogHeader>
                <DialogTitle>
                  {isAddProjectAgentOpen
                    ? "Add agent to Project"
                    : isAddAgentOpen
                      ? "Add agent"
                      : "Edit agent"}
                </DialogTitle>
              </DialogHeader>

              <form
                className="flex min-h-0 flex-1 flex-col gap-4"
                onSubmit={(event) => void handleSaveAgent(event)}
              >
                <div className="flex items-start justify-between gap-4">
                  <label className="flex min-w-0 flex-1 flex-col gap-1.5" htmlFor="agent-name">
                    <span className="font-medium">Agent name</span>
                    <Input
                      id="agent-name"
                      value={newAgentName}
                      onChange={(event) => setNewAgentName(event.currentTarget.value)}
                      placeholder="Agent name"
                      required
                    />
                  </label>

                  <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                    <label className="font-medium" htmlFor="agent-path">
                      Agent path
                    </label>
                    <div className="flex min-w-0 items-center gap-2">
                      <Input
                        id="agent-path"
                        value={newAgentPath}
                        placeholder="Choose an agent directory"
                        title={newAgentPath || "No agent path selected"}
                        readOnly
                        required
                        className="min-w-0 flex-1"
                      />
                      <Button
                        type="button"
                        variant="outline"
                        size="icon-sm"
                        aria-label="Choose agent path"
                        title="Choose agent path"
                        aria-busy={isSelectingAgentPath}
                        onClick={() => void handleSelectAgentPath()}
                        disabled={isSelectingAgentPath || isMutating}
                      >
                        <RiFolderOpenLine aria-hidden="true" />
                      </Button>
                    </div>
                  </div>
                </div>

                <fieldset className="flex min-h-0 flex-1 flex-col">
                  <legend className="sr-only">Skills</legend>
                  <Tabs
                    value={editingAgentSkillTab}
                    onValueChange={(value) => {
                      if (value === "item" || value === "set") {
                        setEditingAgentSkillTab(value);
                      }
                    }}
                    className="min-h-0 flex-1 gap-1.5"
                  >
                    <div className="flex items-end justify-between gap-4">
                      <span className="font-medium">Skills</span>
                      <TabsList className="shrink-0">
                        <TabsTrigger value="item">Item</TabsTrigger>
                        <TabsTrigger value="set">Set</TabsTrigger>
                      </TabsList>
                    </div>

                    <TabsContent value="item" className="flex min-h-0 flex-1 flex-col">
                      <ScrollArea
                        className="min-h-0 flex-1 border border-input"
                        aria-busy={isCatalogSkillsLoading}
                      >
                        {catalogSkills.length > 0 ? (
                          <AgentSkillList
                            skills={catalogSkills}
                            selectedSkillIds={newAgentSkillIds}
                            disabled={isMutating}
                            onToggle={toggleNewAgentSkill}
                          />
                        ) : isCatalogSkillsLoading ? (
                          <EmptyState
                            title="Loading skills…"
                            description="Reading the central Skills list."
                          />
                        ) : catalogSkillsError ? (
                          <ErrorNotice title="Unable to load skills" error={catalogSkillsError} />
                        ) : (
                          <EmptyState
                            title="No catalog skills"
                            description="The central Skills list is empty."
                          />
                        )}
                      </ScrollArea>
                    </TabsContent>

                    <TabsContent value="set" className="flex min-h-0 flex-1 flex-col">
                      <ScrollArea className="min-h-0 flex-1 border border-input">
                        <EmptyState
                          title="Skill sets unavailable"
                          description="Skill set actions are not available yet."
                        />
                      </ScrollArea>
                    </TabsContent>
                  </Tabs>
                </fieldset>

                {mutationError ? (
                  <div role="alert" className="shrink-0 border border-destructive/40 px-3 py-2">
                    <p className="font-medium">Unable to save Agent</p>
                    <p className="text-muted-foreground">
                      {mutationError.message}{" "}
                      <span className="font-mono text-[0.65rem]">({mutationError.code})</span>
                    </p>
                  </div>
                ) : null}

                <DialogFooter className="shrink-0">
                  <DialogClose asChild>
                    <Button type="button" variant="outline">
                      Cancel
                    </Button>
                  </DialogClose>
                  <Button
                    type="submit"
                    aria-busy={isMutating}
                    disabled={
                      isMutating ||
                      isSelectingAgentPath ||
                      !newAgentName.trim() ||
                      !newAgentPath ||
                      ((isAddAgentOpen || isAddProjectAgentOpen) && newAgentSkillIds.size === 0)
                    }
                  >
                    {isMutating
                      ? isAddProjectAgentOpen
                        ? "Copying…"
                        : "Saving…"
                      : isAddAgentOpen || isAddProjectAgentOpen
                        ? "Add agent"
                        : "Save changes"}
                  </Button>
                </DialogFooter>
              </form>
            </>
          ) : null}
        </DialogContent>
      </Dialog>

      <Dialog
        open={isAddProjectOpen}
        onOpenChange={(open) => {
          if (!isMutating) {
            setIsAddProjectOpen(open);
          }
        }}
      >
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>Add project</DialogTitle>
          </DialogHeader>
          <form className="flex flex-col gap-4" onSubmit={(event) => void handleAddProject(event)}>
            <div className="flex items-start justify-between gap-4">
              <label className="flex min-w-0 flex-1 flex-col gap-1.5" htmlFor="project-name">
                <span className="font-medium">Project name</span>
                <Input
                  id="project-name"
                  value={newProjectName}
                  onChange={(event) => setNewProjectName(event.currentTarget.value)}
                  placeholder="Project name"
                  disabled={isMutating}
                  required
                />
              </label>

              <div className="flex min-w-0 flex-1 flex-col gap-1.5">
                <label className="font-medium" htmlFor="project-path">
                  Project path
                </label>
                <div className="flex min-w-0 items-center gap-2">
                  <Input
                    id="project-path"
                    value={newProjectPath}
                    placeholder="Choose a project directory"
                    title={newProjectPath || "No project path selected"}
                    readOnly
                    disabled={isMutating}
                    required
                    className="min-w-0 flex-1"
                  />
                  <Button
                    type="button"
                    variant="outline"
                    size="icon-sm"
                    aria-label="Choose project path"
                    title="Choose project path"
                    aria-busy={isSelectingProjectPath}
                    onClick={() => void handleSelectProjectPath()}
                    disabled={isSelectingProjectPath || isMutating}
                  >
                    <RiFolderOpenLine aria-hidden="true" />
                  </Button>
                </div>
              </div>
            </div>

            <DialogFooter>
              <DialogClose asChild>
                <Button type="button" variant="outline" disabled={isMutating}>
                  Cancel
                </Button>
              </DialogClose>
              <Button
                type="submit"
                aria-busy={isMutating}
                disabled={
                  isMutating || isSelectingProjectPath || !newProjectName.trim() || !newProjectPath
                }
              >
                {isMutating ? "Adding…" : "Add project"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

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
                aria-haspopup="dialog"
                onClick={openAddProjectDialog}
                disabled={isSelectingProjectPath || isMutating}
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
