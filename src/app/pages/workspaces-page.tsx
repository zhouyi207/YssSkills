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

import { skillSets, skills } from "./skills-page";
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

type WorkspaceEntry = {
  slug: string;
  name: string;
  path: string;
  count: number;
};

type AgentSkillTab = "item" | "set";
type WorkspaceTab = "agent" | "project";
type AgentEntry = WorkspaceEntry & {
  skillSlugs: string[];
};

const agents: AgentEntry[] = [
  {
    slug: "claude-code",
    name: "Claude Code",
    path: "~/.claude/skills",
    count: 12,
    skillSlugs: skills.slice(0, 12).map((skill) => skill.slug),
  },
  {
    slug: "codex",
    name: "Codex",
    path: "~/.codex/skills",
    count: 8,
    skillSlugs: skills.slice(0, 8).map((skill) => skill.slug),
  },
  {
    slug: "cursor",
    name: "Cursor",
    path: "~/.cursor/skills",
    count: 4,
    skillSlugs: skills.slice(0, 4).map((skill) => skill.slug),
  },
];

const projects: WorkspaceEntry[] = [
  {
    slug: "yssbi-project",
    name: "YssBI project",
    path: "D:/Projects/YssBI/.agents/skills",
    count: 8,
  },
  {
    slug: "skills-manager",
    name: "Skills Manager",
    path: "D:/Projects/YssSkills/.agents/skills",
    count: 6,
  },
  {
    slug: "shared-skills",
    name: "Shared skills",
    path: "D:/Projects/shared-skills",
    count: 4,
  },
];

function filterEntries(entries: WorkspaceEntry[], query: string) {
  if (!query) {
    return entries;
  }

  return entries.filter((entry) =>
    `${entry.name} ${entry.path} ${entry.count}`.toLowerCase().includes(query),
  );
}

function getDirectoryName(path: string) {
  const segments = path.replace(/[\\/]+$/, "").split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? "Imported project";
}

function WorkspaceList({
  items,
  selectedSlugs,
  onToggle,
  onEdit,
}: {
  items: WorkspaceEntry[];
  selectedSlugs: ReadonlySet<string>;
  onToggle: (slug: string, checked?: boolean) => void;
  onEdit?: (slug: string) => void;
}) {
  const keepCheckboxesVisible = selectedSlugs.size > 0;

  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {items.map((entry) => {
        const isSelected = selectedSlugs.has(entry.slug);

        return (
          <div
            key={entry.slug}
            role="button"
            tabIndex={0}
            aria-pressed={isSelected}
            className="group/workspace-row mx-4 grid min-h-14 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_minmax(0,2fr)_auto_2rem] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50"
            onClick={() => onToggle(entry.slug)}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) {
                return;
              }

              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onToggle(entry.slug);
              }
            }}
          >
            <div className="flex size-6 items-center justify-center">
              <Checkbox
                aria-label={`Select ${entry.name}`}
                checked={isSelected}
                className={cn(
                  "opacity-0 transition-opacity group-hover/workspace-row:opacity-100 group-focus-within/workspace-row:opacity-100",
                  keepCheckboxesVisible && "opacity-100",
                )}
                onClick={(event) => event.stopPropagation()}
                onCheckedChange={(checked) => onToggle(entry.slug, checked === true)}
              />
            </div>
            <div className="flex min-w-0 items-baseline gap-2">
              <h3 className="min-w-0 truncate font-medium">{entry.name}</h3>
            </div>
            <div className="min-w-0 truncate text-muted-foreground">{entry.path}</div>
            <div className="shrink-0 justify-self-end">
              <Badge variant="secondary">{entry.count}</Badge>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={`Edit ${entry.name}`}
              title={`Edit ${entry.name}`}
              className="justify-self-end"
              onClick={(event) => {
                event.stopPropagation();
                onEdit?.(entry.slug);
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

type SelectionListItem = {
  slug: string;
  name: string;
  subtitle: string;
};

function SelectionList({
  items,
  selectedSlugs,
  onToggle,
}: {
  items: ReadonlyArray<SelectionListItem>;
  selectedSlugs: ReadonlySet<string>;
  onToggle: (slug: string, checked: boolean) => void;
}) {
  return (
    <div role="list" className="flex flex-col p-1">
      {items.map((item) => (
        <label
          key={item.slug}
          className="flex min-w-0 cursor-pointer items-center gap-2 px-2 py-1.5 hover:bg-muted"
        >
          <Checkbox
            aria-label={`Select ${item.name}`}
            checked={selectedSlugs.has(item.slug)}
            onCheckedChange={(checked) => onToggle(item.slug, checked === true)}
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

export function WorkspacesPage() {
  const [query, setQuery] = useState("");
  const [agentItems, setAgentItems] = useState(agents);
  const [projectItems, setProjectItems] = useState(projects);
  const [selectedProjectSlug, setSelectedProjectSlug] = useState(projects[0]?.slug ?? "");
  const [selectedSlugs, setSelectedSlugs] = useState<Set<string>>(() => new Set());
  const [activeTab, setActiveTab] = useState<WorkspaceTab>("agent");
  const [isProjectListOpen, setIsProjectListOpen] = useState(false);
  const [isSelectingProject, setIsSelectingProject] = useState(false);
  const [isAddAgentOpen, setIsAddAgentOpen] = useState(false);
  const [editingAgentSlug, setEditingAgentSlug] = useState<string | null>(null);
  const [newAgentName, setNewAgentName] = useState("");
  const [newAgentPath, setNewAgentPath] = useState("");
  const [isSelectingAgentPath, setIsSelectingAgentPath] = useState(false);
  const [newAgentSkillSlugs, setNewAgentSkillSlugs] = useState<Set<string>>(
    () => new Set(),
  );
  const [newAgentSkillTab, setNewAgentSkillTab] = useState<AgentSkillTab>("item");
  const normalizedQuery = query.trim().toLowerCase();

  const toggleSelection = (slug: string, checked?: boolean) => {
    setSelectedSlugs((current) => {
      const next = new Set(current);
      const shouldSelect = checked ?? !next.has(slug);

      if (shouldSelect) {
        next.add(slug);
      } else {
        next.delete(slug);
      }

      return next;
    });
  };

  const filteredAgents = useMemo(
    () => filterEntries(agentItems, normalizedQuery),
    [agentItems, normalizedQuery],
  );
  const filteredProjects = useMemo(
    () => filterEntries(projectItems, normalizedQuery),
    [normalizedQuery, projectItems],
  );

  const selectableSlugs = [...filteredAgents, ...filteredProjects].map((entry) => entry.slug);
  const allVisibleItemsSelected =
    selectableSlugs.length > 0 && selectableSlugs.every((slug) => selectedSlugs.has(slug));
  const projectSlugs = filteredProjects.map((project) => project.slug);
  const allProjectsSelected =
    projectSlugs.length > 0 && projectSlugs.every((slug) => selectedSlugs.has(slug));
  const hasSelectedProjects = projectSlugs.some((slug) => selectedSlugs.has(slug));

  const toggleSelectAll = () => {
    setSelectedSlugs((current) => {
      const next = new Set(current);

      selectableSlugs.forEach((slug) => {
        if (allVisibleItemsSelected) {
          next.delete(slug);
        } else {
          next.add(slug);
        }
      });

      return next;
    });
  };

  const toggleSelectAllProjects = () => {
    setSelectedSlugs((current) => {
      const next = new Set(current);

      projectSlugs.forEach((slug) => {
        if (allProjectsSelected) {
          next.delete(slug);
        } else {
          next.add(slug);
        }
      });

      return next;
    });
  };

  const handleImportProject = async () => {
    setIsSelectingProject(true);

    try {
      const selectedPath = await selectDirectory("Import project");

      if (!selectedPath) {
        return;
      }

      const name = getDirectoryName(selectedPath);
      const slugBase =
        name
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, "-")
          .replace(/^-+|-+$/g, "") || "project";
      const importedProject = {
        slug: `${slugBase}-${Date.now()}`,
        name,
        path: selectedPath,
        count: 0,
      };

      setProjectItems((current) => [...current, importedProject]);
      setSelectedProjectSlug(importedProject.slug);
      toast.success("Project imported.");
    } catch {
      toast.error("Unable to import the project folder.");
    } finally {
      setIsSelectingProject(false);
    }
  };

  const handleTabChange = (value: string) => {
    if (value === "agent" || value === "project") {
      setActiveTab(value);
    }
  };

  const openAddAgentDialog = () => {
    setEditingAgentSlug(null);
    setNewAgentName("");
    setNewAgentPath("");
    setNewAgentSkillSlugs(new Set());
    setNewAgentSkillTab("item");
    setIsAddAgentOpen(true);
  };

  const openEditAgentDialog = (slug: string) => {
    const agent = agentItems.find((item) => item.slug === slug);
    if (!agent) {
      return;
    }

    setEditingAgentSlug(slug);
    setNewAgentName(agent.name);
    setNewAgentPath(agent.path);
    setNewAgentSkillSlugs(new Set(agent.skillSlugs));
    setNewAgentSkillTab("item");
    setIsAddAgentOpen(true);
  };

  const handleAgentSkillTabChange = (value: string) => {
    if (value === "item" || value === "set") {
      setNewAgentSkillTab(value);
    }
  };

  const handleSelectAgentPath = async () => {
    setIsSelectingAgentPath(true);

    try {
      const selectedPath = await selectDirectory("Select agent skills directory");

      if (selectedPath) {
        setNewAgentPath(selectedPath);
      }
    } catch {
      toast.error("Unable to open the agent path picker.");
    } finally {
      setIsSelectingAgentPath(false);
    }
  };

  const toggleNewAgentSkill = (slug: string, checked: boolean) => {
    setNewAgentSkillSlugs((current) => {
      const next = new Set(current);

      if (checked) {
        next.add(slug);
      } else {
        next.delete(slug);
      }

      return next;
    });
  };

  const handleAddAgent = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const name = newAgentName.trim();
    const path = newAgentPath.trim();

    if (!name || !path) {
      toast.error("Enter an agent name and path.");
      return;
    }

    if (newAgentSkillSlugs.size === 0) {
      toast.error("Select at least one skill.");
      return;
    }

    if (editingAgentSlug) {
      setAgentItems((current) =>
        current.map((agent) =>
          agent.slug === editingAgentSlug
            ? {
                ...agent,
                name,
                path,
                count: newAgentSkillSlugs.size,
                skillSlugs: [...newAgentSkillSlugs],
              }
            : agent,
        ),
      );
      setIsAddAgentOpen(false);
      toast.success("Agent updated.");
      return;
    }

    const slugBase =
      name
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "") || "agent";

    setAgentItems((current) => [
      ...current,
      {
        slug: `${slugBase}-${Date.now()}`,
        name,
        path,
        count: newAgentSkillSlugs.size,
        skillSlugs: [...newAgentSkillSlugs],
      },
    ]);
    setIsAddAgentOpen(false);
    toast.success("Agent added.");
  };

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Workspaces</h1>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-pressed={allVisibleItemsSelected}
            onClick={toggleSelectAll}
          >
            <RiCheckboxMultipleLine aria-hidden="true" data-icon="inline-start" />
            {allVisibleItemsSelected ? "Deselect all" : "Select all"}
          </Button>
          <Button type="button" variant="outline" size="sm" onClick={openAddAgentDialog}>
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
          <Button type="button" variant="outline" size="sm">
            <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
            Refresh
          </Button>
        </div>
      </header>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border bg-background">
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
                items={projectItems.map((project) => project.slug)}
                value={selectedProjectSlug}
                onValueChange={(value) => setSelectedProjectSlug(value ?? "")}
                itemToStringValue={(slug) =>
                  projectItems.find((project) => project.slug === slug)?.name ?? slug
                }
              >
                <ComboboxInput
                  aria-label="Project"
                  placeholder="Select project"
                  className="w-48 shrink-0"
                />
                <ComboboxContent>
                  <ComboboxEmpty>No matching projects</ComboboxEmpty>
                  <ComboboxList>
                    {(projectSlug) => {
                      const project = projectItems.find((item) => item.slug === projectSlug);
                      if (!project) {
                        return null;
                      }

                      return (
                        <ComboboxItem key={project.slug} value={project.slug}>
                          <span className="min-w-0 truncate">{project.name}</span>
                        </ComboboxItem>
                      );
                    }}
                  </ComboboxList>
                </ComboboxContent>
              </Combobox>
            ) : null}
            <TabsList className="shrink-0">
              <TabsTrigger value="agent">
                Agent
              </TabsTrigger>
              <TabsTrigger value="project">
                Project
              </TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value="agent" className="flex min-h-0 min-w-0 flex-1 flex-col">
            {filteredAgents.length > 0 ? (
              <ScrollArea className="min-h-0 min-w-0 flex-1">
                <WorkspaceList
                  items={filteredAgents}
                  selectedSlugs={selectedSlugs}
                  onToggle={toggleSelection}
                  onEdit={openEditAgentDialog}
                />
              </ScrollArea>
            ) : (
              <div className="px-4 py-10 text-center" aria-live="polite">
                <p className="text-sm font-medium">No matching results</p>
                <p className="mt-1 text-sm text-muted-foreground">Try a different search term.</p>
              </div>
            )}
          </TabsContent>

          <TabsContent value="project" className="flex min-h-0 min-w-0 flex-1 flex-col">
            {filteredAgents.length > 0 ? (
              <ScrollArea className="min-h-0 min-w-0 flex-1">
                <WorkspaceList
                  items={filteredAgents}
                  selectedSlugs={selectedSlugs}
                  onToggle={toggleSelection}
                  onEdit={openEditAgentDialog}
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

      <Dialog open={isProjectListOpen} onOpenChange={setIsProjectListOpen}>
        <DialogContent className="flex h-[min(80vh,720px)] max-w-2xl flex-col overflow-hidden">
          <DialogHeader>
            <DialogTitle>Project list</DialogTitle>
          </DialogHeader>
          <ScrollArea className="min-h-0 flex-1 border border-input">
            <SelectionList
              items={filteredProjects.map((project) => ({
                slug: project.slug,
                name: project.name,
                subtitle: project.path,
              }))}
              selectedSlugs={selectedSlugs}
              onToggle={toggleSelection}
            />
          </ScrollArea>
          <div className="flex shrink-0 items-center justify-between gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              aria-pressed={allProjectsSelected}
              onClick={toggleSelectAllProjects}
            >
              <RiCheckboxMultipleLine aria-hidden="true" data-icon="inline-start" />
              {allProjectsSelected ? "Deselect all" : "Select all"}
            </Button>
            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => void handleImportProject()}
                disabled={isSelectingProject}
              >
                <RiAddLine aria-hidden="true" data-icon="inline-start" />
                Add
              </Button>
              <Button
                type="button"
                variant="destructive"
                size="sm"
                disabled={!hasSelectedProjects}
              >
                <RiDeleteBinLine aria-hidden="true" data-icon="inline-start" />
                Delete
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={isAddAgentOpen} onOpenChange={setIsAddAgentOpen}>
        <DialogContent className="flex h-[min(80vh,720px)] max-w-2xl flex-col overflow-hidden">
          <DialogHeader>
            <DialogTitle>{editingAgentSlug ? "Edit agent" : "Add agent"}</DialogTitle>
          </DialogHeader>
          <form className="flex min-h-0 flex-1 flex-col gap-4" onSubmit={handleAddAgent}>
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
                    placeholder="Choose an agent skills directory"
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
                    onClick={() => void handleSelectAgentPath()}
                    disabled={isSelectingAgentPath}
                  >
                    <RiFolderOpenLine aria-hidden="true" />
                  </Button>
                </div>
              </div>
            </div>

            <fieldset className="flex min-h-0 flex-1 flex-col">
              <legend className="sr-only">Skills</legend>
              <Tabs
                value={newAgentSkillTab}
                onValueChange={handleAgentSkillTabChange}
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
                  <ScrollArea className="min-h-0 flex-1 border border-input">
                    <SelectionList
                      items={skills.map((skill) => ({
                        slug: skill.slug,
                        name: skill.name,
                        subtitle: skill.source,
                      }))}
                      selectedSlugs={newAgentSkillSlugs}
                      onToggle={toggleNewAgentSkill}
                    />
                  </ScrollArea>
                </TabsContent>

                <TabsContent value="set" className="flex min-h-0 flex-1 flex-col">
                  <ScrollArea className="min-h-0 flex-1 border border-input">
                    <SelectionList
                      items={skillSets.map((skillSet) => ({
                        slug: skillSet.slug,
                        name: skillSet.name,
                        subtitle: skillSet.scope,
                      }))}
                      selectedSlugs={newAgentSkillSlugs}
                      onToggle={toggleNewAgentSkill}
                    />
                  </ScrollArea>
                </TabsContent>
              </Tabs>
            </fieldset>

            <DialogFooter>
              <DialogClose asChild>
                <Button type="button" variant="outline">
                  Cancel
                </Button>
              </DialogClose>
              <Button type="submit">{editingAgentSlug ? "Save changes" : "Add agent"}</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}
