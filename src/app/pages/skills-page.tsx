import { useMemo, useState } from "react";
import {
  RiAddLine,
  RiCheckboxMultipleLine,
  RiDeleteBinLine,
  RiDownloadLine,
  RiEyeLine,
  RiRefreshLine,
  RiSearchLine,
  RiUploadLine,
} from "@remixicon/react";
import ReactMarkdown from "react-markdown";

import { useCatalogSkills } from "@/app/hooks/use-catalog-skills";
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
import type { CatalogSkillSummaryDto, SkillSourceDto } from "@/shared/types/skills";

function formatSkillSource(source: SkillSourceDto) {
  switch (source.kind) {
    case "local":
      return `Local · ${source.path.display}`;
    case "registry":
      return `Registry · ${source.registry}/${source.skill}${source.version ? ` @ ${source.version}` : ""}`;
    case "git":
      return `Git · ${source.url}${source.revision ? ` @ ${source.revision}` : ""}${
        source.subdirectory ? ` · ${source.subdirectory.display}` : ""
      }`;
  }
}

function skillMatchesQuery(skill: CatalogSkillSummaryDto, normalizedQuery: string) {
  if (!normalizedQuery) {
    return true;
  }

  return [
    skill.name,
    skill.description,
    skill.version ?? "",
    formatSkillSource(skill.source),
    skill.location.display,
  ]
    .join(" ")
    .toLowerCase()
    .includes(normalizedQuery);
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
                title={formatSkillSource(skill.source)}
              >
                {formatSkillSource(skill.source)}
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

export function SkillsPage() {
  const {
    data,
    skills,
    error,
    isLoading,
    isRefreshing,
    refresh,
    detail,
    detailError,
    isDetailLoading,
    loadDetail,
    closeDetail,
  } = useCatalogSkills();
  const [query, setQuery] = useState("");
  const [selectedSkillIds, setSelectedSkillIds] = useState<Set<string>>(() => new Set());
  const [activeTab, setActiveTab] = useState<"item" | "set">("item");
  const [viewingSkillId, setViewingSkillId] = useState<string | null>(null);
  const normalizedQuery = query.trim().toLowerCase();

  const filteredSkills = useMemo(
    () => skills.filter((skill) => skillMatchesQuery(skill, normalizedQuery)),
    [normalizedQuery, skills],
  );
  const visibleSkillIds = filteredSkills.map((skill) => skill.id);
  const allVisibleItemsSelected =
    visibleSkillIds.length > 0 && visibleSkillIds.every((skillId) => selectedSkillIds.has(skillId));
  const viewingSkill = skills.find((skill) => skill.id === viewingSkillId) ?? null;
  const visibleDetail = detail?.skill.id === viewingSkillId ? detail : null;

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

  const handleTabChange = (value: string) => {
    if (value === "item" || value === "set") {
      setActiveTab(value);
    }
  };

  const toggleSelectAll = () => {
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

  const handleRefresh = () => {
    void refresh();
  };

  const handleView = (skillId: string) => {
    setViewingSkillId(skillId);
    void loadDetail(skillId);
  };

  const handleCloseDetail = () => {
    setViewingSkillId(null);
    closeDetail();
  };

  const isSelectionUnavailable = activeTab === "set" || visibleSkillIds.length === 0;

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
              aria-pressed={allVisibleItemsSelected}
              aria-label={activeTab === "set" ? "Select all unavailable for skill sets" : undefined}
              title={activeTab === "set" ? "Skill set selection is unavailable" : undefined}
              disabled={isSelectionUnavailable}
              onClick={toggleSelectAll}
            >
              <RiCheckboxMultipleLine aria-hidden="true" data-icon="inline-start" />
              {allVisibleItemsSelected ? "Deselect all" : "Select all"}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-label="Update unavailable"
              title="Skill updates are unavailable"
              disabled
            >
              <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
              Update
            </Button>
            <Button
              type="button"
              variant="destructive"
              size="sm"
              aria-label="Delete unavailable"
              title="Skill deletion is unavailable"
              disabled
            >
              <RiDeleteBinLine aria-hidden="true" data-icon="inline-start" />
              Delete
            </Button>
          </>
          {activeTab === "item" ? (
            <>
              <Button
                type="button"
                variant="outline"
                size="sm"
                aria-label="Import unavailable"
                title="Skill import is unavailable"
                disabled
              >
                <RiDownloadLine aria-hidden="true" data-icon="inline-start" />
                Import
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                aria-label="Export unavailable"
                title="Skill export is unavailable"
                disabled
              >
                <RiUploadLine aria-hidden="true" data-icon="inline-start" />
                Export
              </Button>
            </>
          ) : (
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-label="Add unavailable"
              title="Adding skill sets is unavailable"
              disabled
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
            disabled={isLoading || isRefreshing}
            onClick={handleRefresh}
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
              <TabsTrigger value="set" title="Skill sets are unavailable">
                Set
              </TabsTrigger>
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
                  <p className="text-sm text-muted-foreground">
                    {error.message} <span className="font-mono text-xs">({error.code})</span>
                  </p>
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

            {isLoading && !data ? (
              <div className="px-4 py-10 text-center" role="status" aria-live="polite">
                <p className="text-sm font-medium">Loading catalog skills…</p>
              </div>
            ) : error && !data ? (
              <div className="px-4 py-10 text-center" role="alert">
                <p className="text-sm font-medium">Unable to load catalog skills</p>
                <p className="mt-1 text-sm text-muted-foreground">{error.message}</p>
                <p className="mt-1 font-mono text-xs text-muted-foreground">{error.code}</p>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="mt-4"
                  onClick={handleRefresh}
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
            <div className="px-4 py-10 text-center" aria-live="polite">
              <p className="text-sm font-medium">Skill sets unavailable</p>
              <p className="mt-1 text-sm text-muted-foreground">
                Skill set actions are not available yet.
              </p>
            </div>
          </TabsContent>
        </Tabs>
      </div>

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
                  <p className="mt-1 max-w-xl text-sm text-muted-foreground">
                    {detailError.message}
                  </p>
                  <p className="mt-1 font-mono text-xs text-muted-foreground">{detailError.code}</p>
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
