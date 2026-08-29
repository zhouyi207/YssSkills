import { useMemo, useState } from "react";
import { RiEditLine, RiRefreshLine, RiSearchLine } from "@remixicon/react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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

const agents: WorkspaceEntry[] = [
  {
    slug: "claude-code",
    name: "Claude Code",
    path: "~/.claude/skills",
    count: 12,
  },
  {
    slug: "codex",
    name: "Codex",
    path: "~/.codex/skills",
    count: 8,
  },
  {
    slug: "cursor",
    name: "Cursor",
    path: "~/.cursor/skills",
    count: 4,
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

function WorkspaceList({
  items,
  selectedSlugs,
  onToggle,
}: {
  items: WorkspaceEntry[];
  selectedSlugs: ReadonlySet<string>;
  onToggle: (slug: string, checked?: boolean) => void;
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
            className="group/workspace-row mx-4 grid min-h-14 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_minmax(0,2fr)_2rem] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50 lg:mx-6 lg:px-6"
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
              <span className="max-w-32 shrink-0 truncate text-[0.65rem] text-muted-foreground">
                {entry.slug}
              </span>
              <Badge variant="secondary" className="shrink-0">
                {entry.count}
              </Badge>
            </div>
            <div className="min-w-0 truncate text-muted-foreground">{entry.path}</div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={`Edit ${entry.name}`}
              title={`Edit ${entry.name}`}
              className="justify-self-end"
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

export function WorkspacesPage() {
  const [query, setQuery] = useState("");
  const [selectedSlugs, setSelectedSlugs] = useState<Set<string>>(() => new Set());
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

  const filteredAgents = useMemo(() => filterEntries(agents, normalizedQuery), [normalizedQuery]);
  const filteredProjects = useMemo(
    () => filterEntries(projects, normalizedQuery),
    [normalizedQuery],
  );

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Workspaces</h1>
        <Button type="button" variant="outline" size="sm">
          <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
          Refresh
        </Button>
      </header>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border bg-background">
        <Tabs defaultValue="agent" className="min-h-0 min-w-0 flex-1 gap-0">
          <div className="flex flex-col gap-3 p-4 lg:flex-row lg:items-center lg:justify-between lg:px-6">
            <div className="relative w-full max-w-md">
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
            <TabsList className="w-full lg:w-auto">
              <TabsTrigger value="agent" className="flex-1 lg:flex-none">
                Agent
              </TabsTrigger>
              <TabsTrigger value="project" className="flex-1 lg:flex-none">
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
                />
              </ScrollArea>
            ) : (
              <div className="px-4 py-10 text-center lg:px-6">
                <p className="font-medium">No matching agents</p>
                <p className="mt-1 text-sm text-muted-foreground">Try a different search term.</p>
              </div>
            )}
          </TabsContent>

          <TabsContent value="project" className="flex min-h-0 min-w-0 flex-1 flex-col">
            {filteredProjects.length > 0 ? (
              <ScrollArea className="min-h-0 min-w-0 flex-1">
                <WorkspaceList
                  items={filteredProjects}
                  selectedSlugs={selectedSlugs}
                  onToggle={toggleSelection}
                />
              </ScrollArea>
            ) : (
              <div className="px-4 py-10 text-center lg:px-6">
                <p className="font-medium">No matching projects</p>
                <p className="mt-1 text-sm text-muted-foreground">Try a different search term.</p>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </div>
    </>
  );
}
