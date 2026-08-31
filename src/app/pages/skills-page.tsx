import { useEffect, useMemo, useState } from "react";
import {
  RiAddLine,
  RiCheckboxMultipleLine,
  RiDeleteBinLine,
  RiDownloadLine,
  RiEditLine,
  RiEyeLine,
  RiRefreshLine,
  RiSearchLine,
  RiUploadLine,
} from "@remixicon/react";
import ReactMarkdown from "react-markdown";
import { toast } from "sonner";

import { useCatalogSkills } from "@/app/hooks/use-catalog-skills";
import { selectDirectory } from "@/app/services/directory-picker";
import { formatIpcError, formatUnknownError } from "@/app/services/ipc-error-presentation";
import { countWorkspaceReconcileIssues } from "@/app/services/workspaces-service";
import { IpcErrorDetails } from "@/components/ipc-error-details";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import type {
  CatalogSkillSummaryDto,
  ScanImportFolderResponseDto,
  SkillSetDto,
} from "@/shared/types/skills";

function formatSkillSource(skill: CatalogSkillSummaryDto) {
  return skill.sourceMetadata?.source ?? "";
}

function skillMatchesQuery(skill: CatalogSkillSummaryDto, normalizedQuery: string) {
  if (!normalizedQuery) {
    return true;
  }

  return [
    skill.name,
    skill.description,
    skill.version ?? "",
    formatSkillSource(skill),
    skill.location.display,
  ]
    .join(" ")
    .toLowerCase()
    .includes(normalizedQuery);
}

function skillCanUpdate(skill: CatalogSkillSummaryDto) {
  const metadata = skill.sourceMetadata;
  const sourceType = metadata?.sourceType?.trim().toLowerCase();
  return (
    (sourceType === "github" || sourceType === "git") &&
    Boolean(metadata?.sourceUrl?.trim()) &&
    Boolean(metadata?.skillPath?.trim())
  );
}

function SkillList({
  skills,
  selectedSkillIds,
  onToggle,
  onView,
}: {
  skills: CatalogSkillSummaryDto[];
  selectedSkillIds: ReadonlySet<string>;
  onToggle: (skillId: string, checked?: boolean) => void;
  onView: (skillId: string) => void;
}) {
  const keepCheckboxesVisible = selectedSkillIds.size > 0;

  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {skills.map((skill) => {
        const isSelected = selectedSkillIds.has(skill.id);

        return (
          <div
            key={skill.id}
            role="button"
            tabIndex={0}
            aria-pressed={isSelected}
            className="group/skill-row mx-4 grid min-h-14 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_minmax(0,2fr)_2rem] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50"
            onClick={() => onToggle(skill.id)}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) {
                return;
              }

              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onToggle(skill.id);
              }
            }}
          >
            <div className="flex size-6 items-center justify-center">
              <Checkbox
                aria-label={`Select ${skill.name}`}
                checked={isSelected}
                className={cn(
                  "opacity-0 transition-opacity group-hover/skill-row:opacity-100 group-focus-within/skill-row:opacity-100",
                  keepCheckboxesVisible && "opacity-100",
                )}
                onClick={(event) => event.stopPropagation()}
                onCheckedChange={(checked) => onToggle(skill.id, checked === true)}
              />
            </div>
            <div className="flex min-w-0 items-baseline gap-2">
              <span className="min-w-0 truncate font-medium">{skill.name}</span>
              <span
                className="max-w-32 shrink-0 truncate text-[0.65rem] text-muted-foreground"
                title={formatSkillSource(skill)}
              >
                {formatSkillSource(skill)}
              </span>
            </div>
            <div className="min-w-0 truncate text-muted-foreground">{skill.description}</div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={`View ${skill.name}`}
              title={`View ${skill.name}`}
              className="justify-self-end"
              onClick={(event) => {
                event.stopPropagation();
                onView(skill.id);
              }}
            >
              <RiEyeLine aria-hidden="true" />
            </Button>
          </div>
        );
      })}
    </div>
  );
}

function SkillSetList({
  sets,
  skillsById,
  selectedSetIds,
  onToggle,
  onEdit,
}: {
  sets: SkillSetDto[];
  skillsById: ReadonlyMap<string, CatalogSkillSummaryDto>;
  selectedSetIds: ReadonlySet<string>;
  onToggle: (setId: string, checked?: boolean) => void;
  onEdit: (setId: string) => void;
}) {
  const keepCheckboxesVisible = selectedSetIds.size > 0;

  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {sets.map((set) => {
        const isSelected = selectedSetIds.has(set.id);
        const memberNames = set.skillIds
          .flatMap((skillId) => {
            const skill = skillsById.get(skillId);
            return skill ? [skill.name] : [];
          })
          .join(", ");

        return (
          <div
            key={set.id}
            role="button"
            tabIndex={0}
            aria-pressed={isSelected}
            className="group/set-row mx-4 grid min-h-14 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_minmax(0,2fr)_2rem] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50"
            onClick={() => onToggle(set.id)}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) {
                return;
              }
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onToggle(set.id);
              }
            }}
          >
            <div className="flex size-6 items-center justify-center">
              <Checkbox
                aria-label={`Select ${set.name}`}
                checked={isSelected}
                className={cn(
                  "opacity-0 transition-opacity group-hover/set-row:opacity-100 group-focus-within/set-row:opacity-100",
                  keepCheckboxesVisible && "opacity-100",
                )}
                onClick={(event) => event.stopPropagation()}
                onCheckedChange={(checked) => onToggle(set.id, checked === true)}
              />
            </div>
            <div className="flex min-w-0 items-baseline gap-2">
              <span className="min-w-0 truncate font-medium">{set.name}</span>
              <span className="shrink-0 text-[0.65rem] text-muted-foreground">
                {set.skillIds.length} skill{set.skillIds.length === 1 ? "" : "s"}
              </span>
            </div>
            <div className="min-w-0 truncate text-muted-foreground">
              {memberNames || "No available skills"}
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={`Edit ${set.name}`}
              title={`Edit ${set.name}`}
              className="justify-self-end"
              onClick={(event) => {
                event.stopPropagation();
                onEdit(set.id);
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

function SkillSelectionList({
  skills,
  selectedSkillIds,
  disabled,
  onToggle,
}: {
  skills: CatalogSkillSummaryDto[];
  selectedSkillIds: ReadonlySet<string>;
  disabled: boolean;
  onToggle: (skillId: string, checked: boolean) => void;
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
            aria-label={`${skill.name} set membership`}
            checked={selectedSkillIds.has(skill.id)}
            disabled={disabled}
            onCheckedChange={(checked) => onToggle(skill.id, checked === true)}
          />
          <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
            <span className="min-w-0 flex-1 truncate text-xs font-medium">{skill.name}</span>
            <span className="max-w-40 shrink-0 truncate text-[0.65rem] text-muted-foreground">
              {formatSkillSource(skill)}
            </span>
          </div>
        </label>
      ))}
    </div>
  );
}

export function SkillsPage() {
  const {
    data,
    skills,
    sets,
    indexDiagnostics,
    indexStatus,
    error,
    isLoading,
    isRefreshing,
    refresh,
    detail,
    detailError,
    isDetailLoading,
    loadDetail,
    closeDetail,
    deleteError,
    isDeleting,
    deleteSkills,
    importError,
    isScanningImport,
    isImporting,
    scanImportFolder,
    importLocalSkills,
    clearImportError,
    isExporting,
    exportCatalogSkills,
    setMutationError,
    isSavingSet,
    isDeletingSets,
    createSkillSet,
    updateSkillSet,
    deleteSkillSets,
    clearSetMutationError,
    isUpdatingSkills,
    updateCatalogSkills,
  } = useCatalogSkills();
  const [query, setQuery] = useState("");
  const [selectedSkillIds, setSelectedSkillIds] = useState<Set<string>>(() => new Set());
  const [selectedSetIds, setSelectedSetIds] = useState<Set<string>>(() => new Set());
  const [activeTab, setActiveTab] = useState<"item" | "set">("item");
  const [viewingSkillId, setViewingSkillId] = useState<string | null>(null);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const [isDeleteSetDialogOpen, setIsDeleteSetDialogOpen] = useState(false);
  const [isSetEditorOpen, setIsSetEditorOpen] = useState(false);
  const [editingSetId, setEditingSetId] = useState<string | null>(null);
  const [setName, setSetName] = useState("");
  const [setSkillIds, setSetSkillIds] = useState<Set<string>>(() => new Set());
  const [setSkillQuery, setSetSkillQuery] = useState("");
  const [importPreview, setImportPreview] = useState<ScanImportFolderResponseDto | null>(null);
  const [selectedImportPaths, setSelectedImportPaths] = useState<Set<string>>(() => new Set());
  const [importQuery, setImportQuery] = useState("");
  const normalizedQuery = query.trim().toLowerCase();
  const normalizedImportQuery = importQuery.trim().toLowerCase();
  const normalizedSetSkillQuery = setSkillQuery.trim().toLowerCase();

  useEffect(() => {
    if (deleteError) {
      toast.error(formatIpcError(deleteError));
    }
  }, [deleteError]);

  const filteredSkills = useMemo(
    () => skills.filter((skill) => skillMatchesQuery(skill, normalizedQuery)),
    [normalizedQuery, skills],
  );
  const skillsById = useMemo(() => new Map(skills.map((skill) => [skill.id, skill])), [skills]);
  const filteredSets = useMemo(() => {
    if (!normalizedQuery) {
      return sets;
    }
    return sets.filter((set) =>
      [
        set.name,
        ...set.skillIds.flatMap((skillId) => {
          const skill = skillsById.get(skillId);
          return skill ? [skill.name, skill.description] : [];
        }),
      ]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery),
    );
  }, [normalizedQuery, sets, skillsById]);
  const visibleSkillIds = filteredSkills.map((skill) => skill.id);
  const visibleSetIds = filteredSets.map((set) => set.id);
  const allVisibleItemsSelected =
    visibleSkillIds.length > 0 && visibleSkillIds.every((skillId) => selectedSkillIds.has(skillId));
  const allVisibleSetsSelected =
    visibleSetIds.length > 0 && visibleSetIds.every((setId) => selectedSetIds.has(setId));
  const selectedSetSkillIds = useMemo(
    () => new Set(sets.filter((set) => selectedSetIds.has(set.id)).flatMap((set) => set.skillIds)),
    [selectedSetIds, sets],
  );
  const hasUpdateableSelection = skills.some(
    (skill) =>
      skillCanUpdate(skill) &&
      (activeTab === "item" ? selectedSkillIds.has(skill.id) : selectedSetSkillIds.has(skill.id)),
  );
  const viewingSkill = skills.find((skill) => skill.id === viewingSkillId) ?? null;
  const visibleDetail = detail?.skill.id === viewingSkillId ? detail : null;
  const filteredImportCandidates = useMemo(() => {
    if (!importPreview || !normalizedImportQuery) {
      return importPreview?.candidates ?? [];
    }

    return importPreview.candidates.filter((candidate) =>
      [candidate.name, candidate.description, candidate.version ?? "", candidate.path.display]
        .join(" ")
        .toLowerCase()
        .includes(normalizedImportQuery),
    );
  }, [importPreview, normalizedImportQuery]);
  const selectableImportPaths = useMemo(
    () =>
      filteredImportCandidates.flatMap((candidate) =>
        candidate.path.value ? [candidate.path.value] : [],
      ),
    [filteredImportCandidates],
  );
  const allImportCandidatesSelected =
    selectableImportPaths.length > 0 &&
    selectableImportPaths.every((path) => selectedImportPaths.has(path));
  const filteredSetSkills = useMemo(
    () => skills.filter((skill) => skillMatchesQuery(skill, normalizedSetSkillQuery)),
    [normalizedSetSkillQuery, skills],
  );

  const toggleSelection = (skillId: string, checked?: boolean) => {
    setSelectedSkillIds((current) => {
      const next = new Set(current);
      const shouldSelect = checked ?? !next.has(skillId);

      if (shouldSelect) {
        next.add(skillId);
      } else {
        next.delete(skillId);
      }

      return next;
    });
  };

  const toggleSetSelection = (setId: string, checked?: boolean) => {
    setSelectedSetIds((current) => {
      const next = new Set(current);
      const shouldSelect = checked ?? !next.has(setId);
      if (shouldSelect) {
        next.add(setId);
      } else {
        next.delete(setId);
      }
      return next;
    });
  };

  const handleTabChange = (value: string) => {
    if (value === "item" || value === "set") {
      setActiveTab(value);
    }
  };

  const toggleSelectAll = () => {
    if (activeTab === "set") {
      setSelectedSetIds((current) => {
        const next = new Set(current);
        visibleSetIds.forEach((setId) => {
          if (allVisibleSetsSelected) {
            next.delete(setId);
          } else {
            next.add(setId);
          }
        });
        return next;
      });
      return;
    }
    setSelectedSkillIds((current) => {
      const next = new Set(current);

      visibleSkillIds.forEach((skillId) => {
        if (allVisibleItemsSelected) {
          next.delete(skillId);
        } else {
          next.add(skillId);
        }
      });

      return next;
    });
  };

  const handleRefresh = async () => {
    const outcome = await refresh();
    if (!outcome) {
      return;
    }

    const importedCount = outcome.requested.imported.length;
    const issueCount = countWorkspaceReconcileIssues(outcome);
    if (issueCount > 0) {
      toast.warning(
        `Agent skills refreshed with ${issueCount} diagnostic${issueCount === 1 ? "" : "s"}; imported ${importedCount} new skill${importedCount === 1 ? "" : "s"}.`,
      );
    } else {
      toast.success(
        `Agent skills refreshed; imported ${importedCount} new skill${importedCount === 1 ? "" : "s"}.`,
      );
    }
  };

  const handleView = (skillId: string) => {
    setViewingSkillId(skillId);
    void loadDetail(skillId);
  };

  const handleCloseDetail = () => {
    setViewingSkillId(null);
    closeDetail();
  };

  const handleDelete = async () => {
    const requestedIds = Array.from(selectedSkillIds);
    const deletedIds = await deleteSkills(requestedIds);
    if (!deletedIds) {
      return;
    }

    const deletedSet = new Set(deletedIds);
    setSelectedSkillIds((current) => {
      const next = new Set(current);
      deletedSet.forEach((skillId) => next.delete(skillId));
      return next;
    });
    if (viewingSkillId && deletedSet.has(viewingSkillId)) {
      handleCloseDetail();
    }
    setIsDeleteDialogOpen(false);
    toast.success(
      `Deleted ${deletedIds.length} skill${deletedIds.length === 1 ? "" : "s"} from the catalog and Agent directories.`,
    );
  };

  const handleUpdate = async () => {
    const outcome = await updateCatalogSkills({
      skillIds: activeTab === "item" ? Array.from(selectedSkillIds) : [],
      setIds: activeTab === "set" ? Array.from(selectedSetIds) : [],
    });
    if (!outcome) {
      return;
    }
    const summary = [
      `${outcome.updatedSkillIds.length} updated`,
      `${outcome.unchangedSkillIds.length} unchanged`,
      `${outcome.unavailableSkillIds.length} without update metadata`,
      `${outcome.failures.length} failed`,
    ].join(" · ");
    if (outcome.unavailableSkillIds.length > 0 || outcome.failures.length > 0) {
      const firstFailure = outcome.failures[0];
      toast.warning(
        firstFailure ? `${summary}. ${firstFailure.name}: ${firstFailure.message}` : summary,
      );
    } else {
      toast.success(summary);
    }
  };

  const openCreateSetDialog = () => {
    clearSetMutationError();
    setEditingSetId(null);
    setSetName("");
    setSetSkillIds(new Set());
    setSetSkillQuery("");
    setIsSetEditorOpen(true);
  };

  const openEditSetDialog = (setId: string) => {
    const set = sets.find((candidate) => candidate.id === setId);
    if (!set) {
      return;
    }
    clearSetMutationError();
    setEditingSetId(set.id);
    setSetName(set.name);
    setSetSkillIds(new Set(set.skillIds));
    setSetSkillQuery("");
    setIsSetEditorOpen(true);
  };

  const toggleSetSkill = (skillId: string, checked: boolean) => {
    setSetSkillIds((current) => {
      const next = new Set(current);
      if (checked) {
        next.add(skillId);
      } else {
        next.delete(skillId);
      }
      return next;
    });
  };

  const handleSaveSet = async () => {
    const name = setName.trim();
    if (!name) {
      toast.error("Enter a Skill Set name.");
      return;
    }
    if (setSkillIds.size === 0) {
      toast.error("Select at least one Skill.");
      return;
    }
    const request = { name, skillIds: Array.from(setSkillIds) };
    const saved = editingSetId
      ? await updateSkillSet({ setId: editingSetId, ...request })
      : await createSkillSet(request);
    if (!saved) {
      return;
    }
    setIsSetEditorOpen(false);
    toast.success(`${editingSetId ? "Updated" : "Created"} Skill Set ${saved.name}.`);
  };

  const handleDeleteSets = async () => {
    const deletedIds = await deleteSkillSets({ setIds: Array.from(selectedSetIds) });
    if (!deletedIds) {
      return;
    }
    const deleted = new Set(deletedIds);
    setSelectedSetIds((current) => {
      const next = new Set(current);
      deleted.forEach((setId) => next.delete(setId));
      return next;
    });
    setIsDeleteSetDialogOpen(false);
    toast.success(
      `Deleted ${deletedIds.length} Skill Set definition${deletedIds.length === 1 ? "" : "s"}; catalog Skills were not deleted.`,
    );
  };

  const closeImportDialog = () => {
    setImportPreview(null);
    setSelectedImportPaths(new Set());
    setImportQuery("");
    clearImportError();
  };

  const handleChooseImportFolder = async () => {
    clearImportError();
    try {
      const root = await selectDirectory("Select a folder containing skills");
      if (!root) {
        return;
      }

      const preview = await scanImportFolder(root);
      setImportPreview(preview);
      setSelectedImportPaths(
        new Set(
          preview.candidates.flatMap((candidate) =>
            candidate.path.value ? [candidate.path.value] : [],
          ),
        ),
      );
    } catch (caught: unknown) {
      toast.error(formatUnknownError(caught, "Unable to scan the selected folder."));
    }
  };

  const toggleImportCandidate = (path: string, checked?: boolean) => {
    setSelectedImportPaths((current) => {
      const next = new Set(current);
      const shouldSelect = checked ?? !next.has(path);
      if (shouldSelect) {
        next.add(path);
      } else {
        next.delete(path);
      }
      return next;
    });
  };

  const toggleAllImportCandidates = () => {
    setSelectedImportPaths((current) => {
      const next = new Set(current);
      selectableImportPaths.forEach((path) => {
        if (allImportCandidatesSelected) {
          next.delete(path);
        } else {
          next.add(path);
        }
      });
      return next;
    });
  };

  const handleImport = async () => {
    const root = importPreview?.root.value;
    if (!importPreview || !root) {
      toast.error("The selected folder path cannot be imported on this platform.");
      return;
    }
    const paths = importPreview.candidates.flatMap((candidate) => {
      const path = candidate.path.value;
      return path && selectedImportPaths.has(path) ? [path] : [];
    });
    if (paths.length === 0) {
      return;
    }

    try {
      const outcome = await importLocalSkills({ root, paths });
      closeImportDialog();
      const importedCount = outcome.importedSkillIds.length;
      const skippedCount = outcome.skippedPaths.length;
      if (skippedCount > 0) {
        toast.warning(
          `Imported ${importedCount} skill${importedCount === 1 ? "" : "s"}; skipped ${skippedCount} already present or conflicting with a catalog directory.`,
        );
      } else {
        toast.success(`Imported ${importedCount} skill${importedCount === 1 ? "" : "s"}.`);
      }
    } catch (caught: unknown) {
      toast.error(formatUnknownError(caught, "Unable to import the selected skills."));
    }
  };

  const handleExport = async () => {
    const skillIds = Array.from(selectedSkillIds);
    if (skillIds.length === 0) {
      return;
    }

    try {
      const destinationRoot = await selectDirectory("Select export destination folder");
      if (!destinationRoot) {
        return;
      }
      const outcome = await exportCatalogSkills({ destinationRoot, skillIds });
      const exportedCount = outcome.exportedSkillIds.length;
      toast.success(
        `Exported ${exportedCount} skill${exportedCount === 1 ? "" : "s"} to ${outcome.exportRoot.display}.`,
      );
    } catch (caught: unknown) {
      toast.error(formatUnknownError(caught, "Unable to export the selected skills."));
    }
  };

  const isSelectionUnavailable =
    activeTab === "item" ? visibleSkillIds.length === 0 : visibleSetIds.length === 0;
  const allVisibleSelected =
    activeTab === "item" ? allVisibleItemsSelected : allVisibleSetsSelected;
  const hasActiveSelection =
    activeTab === "item" ? selectedSkillIds.size > 0 : selectedSetIds.size > 0;

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Skills</h1>
        <div className="flex shrink-0 items-center gap-2">
          <>
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-pressed={allVisibleSelected}
              disabled={isSelectionUnavailable}
              onClick={toggleSelectAll}
            >
              <RiCheckboxMultipleLine aria-hidden="true" data-icon="inline-start" />
              {allVisibleSelected ? "Deselect all" : "Select all"}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-label="Update selected Skills"
              title={
                hasUpdateableSelection
                  ? "Update selected lock-backed Skills"
                  : "Select at least one Skill with source metadata"
              }
              aria-busy={isUpdatingSkills}
              disabled={
                !hasUpdateableSelection ||
                isUpdatingSkills ||
                isDeleting ||
                isDeletingSets ||
                isSavingSet
              }
              onClick={() => void handleUpdate()}
            >
              <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
              {isUpdatingSkills ? "Updating…" : "Update"}
            </Button>
            <Button
              type="button"
              variant="destructive"
              size="sm"
              aria-label={activeTab === "item" ? "Delete selected skills" : "Delete selected sets"}
              disabled={
                !hasActiveSelection ||
                isUpdatingSkills ||
                isDeleting ||
                isDeletingSets ||
                isScanningImport ||
                isImporting ||
                isExporting
              }
              onClick={() => {
                if (activeTab === "item") {
                  setIsDeleteDialogOpen(true);
                } else {
                  clearSetMutationError();
                  setIsDeleteSetDialogOpen(true);
                }
              }}
            >
              <RiDeleteBinLine aria-hidden="true" data-icon="inline-start" />
              {isDeleting || isDeletingSets ? "Deleting…" : "Delete"}
            </Button>
          </>
          {activeTab === "item" ? (
            <>
              <Button
                type="button"
                variant="outline"
                size="sm"
                aria-busy={isScanningImport}
                title="Import skills from a local folder"
                disabled={
                  isLoading ||
                  isRefreshing ||
                  isUpdatingSkills ||
                  isDeleting ||
                  isScanningImport ||
                  isImporting ||
                  isExporting
                }
                onClick={() => void handleChooseImportFolder()}
              >
                <RiDownloadLine aria-hidden="true" data-icon="inline-start" />
                {isScanningImport ? "Scanning…" : "Import"}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                aria-label="Export selected skills"
                aria-busy={isExporting}
                title="Export selected skills to a local folder"
                disabled={
                  selectedSkillIds.size === 0 ||
                  isLoading ||
                  isUpdatingSkills ||
                  isRefreshing ||
                  isDeleting ||
                  isScanningImport ||
                  isImporting ||
                  isExporting
                }
                onClick={() => void handleExport()}
              >
                <RiUploadLine aria-hidden="true" data-icon="inline-start" />
                {isExporting ? "Exporting…" : "Export"}
              </Button>
            </>
          ) : (
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-label="Add Skill Set"
              disabled={isUpdatingSkills || isSavingSet || isDeletingSets || skills.length === 0}
              onClick={openCreateSetDialog}
            >
              <RiAddLine aria-hidden="true" data-icon="inline-start" />
              Add
            </Button>
          )}
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-busy={isRefreshing}
            title="Scan agent skills and refresh the central catalog"
            disabled={
              isLoading ||
              isRefreshing ||
              isUpdatingSkills ||
              isDeleting ||
              isScanningImport ||
              isImporting ||
              isExporting
            }
            onClick={() => void handleRefresh()}
          >
            <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
            Refresh
          </Button>
        </div>
      </header>

      <div
        className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border bg-background"
        aria-busy={isLoading || isRefreshing}
      >
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
                aria-label="Search items and sets"
                value={query}
                onChange={(event) => setQuery(event.currentTarget.value)}
                placeholder="Search items and sets"
                className="pl-8"
              />
            </div>
            <TabsList className="shrink-0">
              <TabsTrigger value="item">Item</TabsTrigger>
              <TabsTrigger value="set">Set</TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value="item" className="flex min-h-0 min-w-0 flex-1 flex-col">
            {error && data ? (
              <div
                role="alert"
                className="mx-4 mb-4 flex flex-wrap items-center justify-between gap-3 border px-4 py-3"
              >
                <div className="min-w-0">
                  <p className="text-sm font-medium">Unable to refresh the catalog</p>
                  <IpcErrorDetails error={error} compact />
                </div>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={isRefreshing}
                  onClick={() => void handleRefresh()}
                >
                  <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                  Retry
                </Button>
              </div>
            ) : null}

            {indexStatus && indexStatus.freshness !== "fresh" ? (
              <div className="mx-4 mb-4 border px-4 py-3 text-sm" role="status" aria-live="polite">
                Showing the saved Skill index while filesystem changes are
                {indexStatus.freshness === "revalidating"
                  ? " checked in the background."
                  : " waiting for background verification."}
              </div>
            ) : null}

            {indexDiagnostics.length > 0 ? (
              <div className="mx-4 mb-4 border px-4 py-3" role="alert">
                <p className="text-sm font-medium">
                  {indexDiagnostics.length} invalid Skill
                  {indexDiagnostics.length === 1 ? " was" : "s were"} excluded
                </p>
                <ul className="mt-2 space-y-1 text-xs text-muted-foreground">
                  {indexDiagnostics.map((diagnostic) => (
                    <li key={diagnostic.skillId}>
                      <span className="font-medium text-foreground">{diagnostic.path.display}</span>
                      {` · ${diagnostic.kind} · ${diagnostic.message}`}
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            {isLoading && !data ? (
              <div className="px-4 py-10 text-center" role="status" aria-live="polite">
                <p className="text-sm font-medium">Loading catalog skills…</p>
              </div>
            ) : error && !data ? (
              <div className="px-4 py-10 text-center" role="alert">
                <p className="text-sm font-medium">Unable to load catalog skills</p>
                <IpcErrorDetails error={error} className="mt-1" />
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="mt-4"
                  onClick={() => void handleRefresh()}
                >
                  <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                  Retry
                </Button>
              </div>
            ) : skills.length === 0 ? (
              <div className="px-4 py-10 text-center" aria-live="polite">
                <p className="text-sm font-medium">No catalog skills</p>
                <p className="mt-1 text-sm text-muted-foreground">
                  The catalog does not contain any skills.
                </p>
              </div>
            ) : filteredSkills.length > 0 ? (
              <ScrollArea className="min-h-0 min-w-0 flex-1">
                <SkillList
                  skills={filteredSkills}
                  selectedSkillIds={selectedSkillIds}
                  onToggle={toggleSelection}
                  onView={handleView}
                />
              </ScrollArea>
            ) : (
              <div className="px-4 py-10 text-center" aria-live="polite">
                <p className="text-sm font-medium">No matching results</p>
                <p className="mt-1 text-sm text-muted-foreground">Try a different search term.</p>
              </div>
            )}
          </TabsContent>

          <TabsContent value="set" className="flex min-h-0 min-w-0 flex-1 flex-col">
            {isLoading && !data ? (
              <div className="px-4 py-10 text-center" role="status" aria-live="polite">
                <p className="text-sm font-medium">Loading Skill Sets…</p>
              </div>
            ) : error && !data ? (
              <div className="px-4 py-10 text-center" role="alert">
                <p className="text-sm font-medium">Unable to load Skill Sets</p>
                <IpcErrorDetails error={error} className="mt-1" />
              </div>
            ) : sets.length === 0 ? (
              <div className="px-4 py-10 text-center" aria-live="polite">
                <p className="text-sm font-medium">No Skill Sets</p>
                <p className="mt-1 text-sm text-muted-foreground">
                  Add a Set to group Skills for quick selection.
                </p>
              </div>
            ) : filteredSets.length > 0 ? (
              <ScrollArea className="min-h-0 min-w-0 flex-1">
                <SkillSetList
                  sets={filteredSets}
                  skillsById={skillsById}
                  selectedSetIds={selectedSetIds}
                  onToggle={toggleSetSelection}
                  onEdit={openEditSetDialog}
                />
              </ScrollArea>
            ) : (
              <div className="px-4 py-10 text-center" aria-live="polite">
                <p className="text-sm font-medium">No matching results</p>
                <p className="mt-1 text-sm text-muted-foreground">Try a different search term.</p>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </div>

      <Dialog
        open={importPreview !== null}
        onOpenChange={(open) => {
          if (!open && !isImporting) {
            closeImportDialog();
          }
        }}
      >
        <DialogContent className="flex h-[min(80vh,720px)] max-w-3xl flex-col gap-0 overflow-hidden">
          {importPreview ? (
            <>
              <DialogHeader className="mb-4">
                <DialogTitle>Select skills to import</DialogTitle>
              </DialogHeader>

              {importPreview.candidates.length > 0 ? (
                <div className="flex w-full min-w-0 items-center gap-2 p-4">
                  <div className="relative min-w-0 flex-1">
                    <RiSearchLine
                      aria-hidden="true"
                      className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
                    />
                    <Input
                      id="import-skill-search"
                      aria-label="Search skills to import"
                      value={importQuery}
                      onChange={(event) => setImportQuery(event.currentTarget.value)}
                      placeholder="Search skills"
                      className="pl-8"
                    />
                  </div>
                  <Button
                    type="button"
                    variant="outline"
                    aria-pressed={allImportCandidatesSelected}
                    disabled={isImporting || selectableImportPaths.length === 0}
                    onClick={toggleAllImportCandidates}
                  >
                    <RiCheckboxMultipleLine aria-hidden="true" data-icon="inline-start" />
                    {allImportCandidatesSelected ? "Deselect all" : "Select all"}
                  </Button>
                </div>
              ) : null}

              <ScrollArea className="min-h-0 flex-1">
                <div className="space-y-2 px-4">
                  {filteredImportCandidates.length > 0 ? (
                    filteredImportCandidates.map((candidate) => {
                      const path = candidate.path.value;
                      return (
                        <label
                          key={candidate.path.display}
                          className={cn(
                            "flex items-start gap-3 border px-3 py-3 transition-colors",
                            path
                              ? "cursor-pointer hover:bg-muted/50"
                              : "cursor-not-allowed opacity-60",
                          )}
                        >
                          <Checkbox
                            aria-label={`Import ${candidate.name}`}
                            checked={path ? selectedImportPaths.has(path) : false}
                            disabled={!path}
                            onCheckedChange={(checked) => {
                              if (path) {
                                toggleImportCandidate(path, checked === true);
                              }
                            }}
                          />
                          <span className="min-w-0 flex-1">
                            <span className="flex min-w-0 items-baseline gap-2">
                              <span className="truncate font-medium">{candidate.name}</span>
                              {candidate.version ? (
                                <span className="shrink-0 text-muted-foreground">
                                  v{candidate.version}
                                </span>
                              ) : null}
                            </span>
                            <span className="mt-1 block text-muted-foreground">
                              {candidate.description}
                            </span>
                            <span
                              className="mt-1 block truncate font-mono text-[0.65rem] text-muted-foreground"
                              title={candidate.path.display}
                            >
                              {candidate.path.display}
                            </span>
                            {!path ? (
                              <span className="mt-1 block text-destructive">
                                This path cannot be represented losslessly and cannot be imported.
                              </span>
                            ) : null}
                          </span>
                        </label>
                      );
                    })
                  ) : (
                    <div className="border px-4 py-10 text-center">
                      <p className="text-sm font-medium">
                        {importPreview.candidates.length > 0
                          ? "No matching results"
                          : "No skills found"}
                      </p>
                      <p className="mt-1 text-muted-foreground">
                        {importPreview.candidates.length > 0
                          ? "Try a different search term."
                          : "The selected folder does not contain a valid SKILL.md."}
                      </p>
                    </div>
                  )}

                  {importPreview.diagnostics.length > 0 ? (
                    <div role="alert" className="border border-destructive/40 px-3 py-3">
                      <p className="font-medium">
                        {importPreview.diagnostics.length} skill
                        {importPreview.diagnostics.length === 1 ? "" : "s"} could not be scanned
                      </p>
                      <div className="mt-2 space-y-2">
                        {importPreview.diagnostics.map((diagnostic) => (
                          <div key={diagnostic.path.display} className="min-w-0">
                            <p
                              className="truncate font-mono text-[0.65rem]"
                              title={diagnostic.path.display}
                            >
                              {diagnostic.path.display}
                            </p>
                            <IpcErrorDetails error={diagnostic.error} compact />
                          </div>
                        ))}
                      </div>
                    </div>
                  ) : null}
                </div>
              </ScrollArea>

              {importError ? (
                <div role="alert" className="mt-4 border border-destructive/40 px-3 py-2">
                  <p className="font-medium">Unable to import the selected skills</p>
                  <IpcErrorDetails error={importError} compact />
                </div>
              ) : null}

              <DialogFooter className="mt-4 px-4">
                <DialogClose asChild>
                  <Button type="button" variant="outline" disabled={isImporting}>
                    Cancel
                  </Button>
                </DialogClose>
                <Button
                  type="button"
                  disabled={
                    isImporting || selectedImportPaths.size === 0 || !importPreview.root.value
                  }
                  onClick={() => void handleImport()}
                >
                  {isImporting
                    ? "Importing…"
                    : `Import ${selectedImportPaths.size} skill${selectedImportPaths.size === 1 ? "" : "s"}`}
                </Button>
              </DialogFooter>
            </>
          ) : null}
        </DialogContent>
      </Dialog>

      <Dialog
        open={isSetEditorOpen}
        onOpenChange={(open) => {
          if (!isSavingSet) {
            setIsSetEditorOpen(open);
            if (!open) {
              clearSetMutationError();
            }
          }
        }}
      >
        <DialogContent className="flex h-[min(80vh,720px)] max-w-2xl flex-col overflow-hidden">
          <DialogHeader>
            <DialogTitle>{editingSetId ? "Edit Skill Set" : "Add Skill Set"}</DialogTitle>
          </DialogHeader>
          <form
            className="flex min-h-0 flex-1 flex-col gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              void handleSaveSet();
            }}
          >
            <label className="flex flex-col gap-1.5" htmlFor="skill-set-name">
              <span className="font-medium">Set name</span>
              <Input
                id="skill-set-name"
                value={setName}
                onChange={(event) => setSetName(event.currentTarget.value)}
                placeholder="Set name"
                maxLength={120}
                disabled={isSavingSet}
                required
              />
            </label>

            <fieldset className="flex min-h-0 flex-1 flex-col gap-2">
              <legend className="font-medium">Skills</legend>
              <div className="relative min-w-0">
                <RiSearchLine
                  aria-hidden="true"
                  className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
                />
                <Input
                  aria-label="Search Skills for Set"
                  value={setSkillQuery}
                  onChange={(event) => setSetSkillQuery(event.currentTarget.value)}
                  placeholder="Search Skills"
                  className="pl-8"
                  disabled={isSavingSet}
                />
              </div>
              <ScrollArea className="min-h-0 flex-1 border border-input">
                {filteredSetSkills.length > 0 ? (
                  <SkillSelectionList
                    skills={filteredSetSkills}
                    selectedSkillIds={setSkillIds}
                    disabled={isSavingSet}
                    onToggle={toggleSetSkill}
                  />
                ) : (
                  <div className="px-4 py-10 text-center text-sm text-muted-foreground">
                    No matching Skills.
                  </div>
                )}
              </ScrollArea>
            </fieldset>

            {setMutationError ? (
              <div role="alert" className="shrink-0 border border-destructive/40 px-3 py-2">
                <p className="font-medium">Unable to save Skill Set</p>
                <IpcErrorDetails error={setMutationError} compact />
              </div>
            ) : null}

            <DialogFooter className="shrink-0">
              <DialogClose asChild>
                <Button type="button" variant="outline" disabled={isSavingSet}>
                  Cancel
                </Button>
              </DialogClose>
              <Button
                type="submit"
                aria-busy={isSavingSet}
                disabled={isSavingSet || !setName.trim() || setSkillIds.size === 0}
              >
                {isSavingSet ? "Saving…" : editingSetId ? "Save changes" : "Add Set"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog
        open={isDeleteSetDialogOpen}
        onOpenChange={(open) => {
          if (!isDeletingSets) {
            setIsDeleteSetDialogOpen(open);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete selected Skill Sets?</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">
            This deletes {selectedSetIds.size} Set definition
            {selectedSetIds.size === 1 ? "" : "s"}. The Skills in each Set remain in the central
            catalog and Agent directories.
          </p>
          {setMutationError ? <IpcErrorDetails error={setMutationError} /> : null}
          <DialogFooter>
            <DialogClose asChild>
              <Button type="button" variant="outline" disabled={isDeletingSets}>
                Cancel
              </Button>
            </DialogClose>
            <Button
              type="button"
              variant="destructive"
              disabled={isDeletingSets}
              onClick={() => void handleDeleteSets()}
            >
              {isDeletingSets ? "Deleting…" : "Delete definitions"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={isDeleteDialogOpen}
        onOpenChange={(open) => {
          if (!isDeleting) {
            setIsDeleteDialogOpen(open);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete selected skills?</DialogTitle>
          </DialogHeader>
          <p className="text-sm text-muted-foreground">
            This permanently deletes {selectedSkillIds.size} selected skill
            {selectedSkillIds.size === 1 ? "" : "s"} from the central catalog and every detected
            Agent skills directory.
          </p>
          <DialogFooter>
            <DialogClose asChild>
              <Button type="button" variant="outline" disabled={isDeleting}>
                Cancel
              </Button>
            </DialogClose>
            <Button
              type="button"
              variant="destructive"
              disabled={isDeleting}
              onClick={() => void handleDelete()}
            >
              {isDeleting ? "Deleting…" : "Delete permanently"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={viewingSkillId !== null}
        onOpenChange={(open) => {
          if (!open) {
            handleCloseDetail();
          }
        }}
      >
        <DialogContent className="flex h-[min(80vh,720px)] max-w-2xl flex-col overflow-hidden">
          {viewingSkillId ? (
            <>
              <DialogHeader>
                <DialogTitle>
                  {visibleDetail?.skill.name ?? viewingSkill?.name ?? "Skill details"}
                </DialogTitle>
              </DialogHeader>
              {isDetailLoading ? (
                <div
                  role="status"
                  aria-live="polite"
                  className="flex min-h-0 flex-1 items-center justify-center py-10 text-center"
                >
                  <p className="text-sm text-muted-foreground">Loading skill details…</p>
                </div>
              ) : detailError ? (
                <div
                  role="alert"
                  className="flex min-h-0 flex-1 flex-col items-center justify-center py-10 text-center"
                >
                  <p className="text-sm font-medium">Unable to load skill details</p>
                  <IpcErrorDetails error={detailError} className="mt-1 max-w-xl" />
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="mt-4"
                    onClick={() => void loadDetail(viewingSkillId)}
                  >
                    <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                    Retry
                  </Button>
                </div>
              ) : visibleDetail ? (
                <ScrollArea className="min-h-0 flex-1">
                  <div className="pr-4 text-sm/relaxed [&_h2]:mb-2 [&_h2]:font-heading [&_h2]:text-sm [&_h2]:font-medium [&_li]:text-muted-foreground [&_p]:text-muted-foreground [&_strong]:font-medium [&_ul]:list-disc [&_ul]:space-y-1 [&_ul]:pl-5">
                    <ReactMarkdown>{visibleDetail.body}</ReactMarkdown>
                  </div>
                </ScrollArea>
              ) : (
                <div
                  role="status"
                  aria-live="polite"
                  className="flex min-h-0 flex-1 items-center justify-center py-10 text-center"
                >
                  <p className="text-sm text-muted-foreground">Skill details are unavailable.</p>
                </div>
              )}
              <DialogFooter>
                <DialogClose asChild>
                  <Button type="button">Close</Button>
                </DialogClose>
              </DialogFooter>
            </>
          ) : null}
        </DialogContent>
      </Dialog>
    </>
  );
}
