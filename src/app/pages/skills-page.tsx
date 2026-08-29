import { useMemo, useState } from "react";
import {
  RiCommandLine,
  RiDatabase2Line,
  RiEditLine,
  RiFileTextLine,
  RiRefreshLine,
  RiSearchLine,
  RiStackLine,
  RiTerminalLine,
} from "@remixicon/react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";

export const skills = [
  {
    name: "Rust patterns",
    slug: "rust-patterns",
    description: "Ownership, error handling, traits, and concurrency patterns.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated today",
    status: "Ready",
    icon: RiTerminalLine,
  },
  {
    name: "Test-driven development",
    slug: "test-driven-development",
    description: "A red-green-refactor loop for changes that stay verifiable.",
    source: "Local",
    scope: "YssBI",
    updated: "Updated yesterday",
    status: "Syncing",
    icon: RiFileTextLine,
  },
  {
    name: "Systematic debugging",
    slug: "systematic-debugging",
    description: "Evidence-first isolation of bugs, failures, and regressions.",
    source: "Local",
    scope: "YssBI",
    updated: "Updated 2 days ago",
    status: "Review",
    icon: RiCommandLine,
  },
  {
    name: "Frontend design",
    slug: "frontend-design",
    description: "A visual language for purposeful, non-template interfaces.",
    source: "Registry",
    scope: "Global",
    updated: "Updated 4 days ago",
    status: "Ready",
    icon: RiDatabase2Line,
  },
  {
    name: "Codebase design",
    slug: "codebase-design",
    description: "Shared vocabulary for clear module boundaries and interfaces.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 5 days ago",
    status: "Ready",
    icon: RiStackLine,
  },
  {
    name: "Software development process",
    slug: "software-dev-process",
    description: "Practical standards for architecture, testing, and delivery.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 6 days ago",
    status: "Ready",
    icon: RiCommandLine,
  },
  {
    name: "Verification before completion",
    slug: "verification-before-completion",
    description: "Evidence-based checks before declaring work complete.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 1 week ago",
    status: "Ready",
    icon: RiFileTextLine,
  },
  {
    name: "Requesting code review",
    slug: "requesting-code-review",
    description: "A focused checklist for getting useful feedback before merging.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 1 week ago",
    status: "Ready",
    icon: RiFileTextLine,
  },
  {
    name: "Receiving code review",
    slug: "receiving-code-review",
    description: "Technical rigor for evaluating and applying review feedback.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 8 days ago",
    status: "Review",
    icon: RiTerminalLine,
  },
  {
    name: "Writing plans",
    slug: "writing-plans",
    description: "Step-by-step implementation plans for multi-part changes.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 9 days ago",
    status: "Ready",
    icon: RiFileTextLine,
  },
  {
    name: "Subagent-driven development",
    slug: "subagent-driven-development",
    description: "A workflow for coordinating focused implementation tasks.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 10 days ago",
    status: "Syncing",
    icon: RiStackLine,
  },
  {
    name: "Dispatching parallel agents",
    slug: "dispatching-parallel-agents",
    description: "Patterns for delegating independent work safely and efficiently.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 11 days ago",
    status: "Ready",
    icon: RiCommandLine,
  },
  {
    name: "Using git worktrees",
    slug: "using-git-worktrees",
    description: "Isolated workspace practices for parallel feature development.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 12 days ago",
    status: "Ready",
    icon: RiDatabase2Line,
  },
  {
    name: "Finishing a development branch",
    slug: "finishing-a-development-branch",
    description: "Options for reviewing and integrating completed changes.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 2 weeks ago",
    status: "Ready",
    icon: RiStackLine,
  },
  {
    name: "React best practices",
    slug: "vercel-react-best-practices",
    description: "Performance guidance for React and Next.js applications.",
    source: "Registry",
    scope: "Global",
    updated: "Updated 2 weeks ago",
    status: "Ready",
    icon: RiCommandLine,
  },
  {
    name: "React composition patterns",
    slug: "vercel-composition-patterns",
    description: "Flexible component APIs that scale without prop proliferation.",
    source: "Registry",
    scope: "Global",
    updated: "Updated 15 days ago",
    status: "Review",
    icon: RiStackLine,
  },
  {
    name: "Tiptap integration",
    slug: "tiptap",
    description: "Patterns for extending and integrating the Tiptap editor.",
    source: "Registry",
    scope: "Global",
    updated: "Updated 16 days ago",
    status: "Ready",
    icon: RiFileTextLine,
  },
  {
    name: "Shadcn components",
    slug: "shadcn",
    description: "Guidance for adding, styling, and composing shadcn components.",
    source: "Registry",
    scope: "YssBI",
    updated: "Updated 17 days ago",
    status: "Syncing",
    icon: RiCommandLine,
  },
  {
    name: "Brainstorming",
    slug: "brainstorming",
    description: "Clarify intent, requirements, and design before implementation.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 3 weeks ago",
    status: "Ready",
    icon: RiFileTextLine,
  },
  {
    name: "Canvas design",
    slug: "canvas-design",
    description: "Design principles for creating original visual documents.",
    source: "Registry",
    scope: "Global",
    updated: "Updated 3 weeks ago",
    status: "Ready",
    icon: RiDatabase2Line,
  },
  {
    name: "Create skill",
    slug: "create-skill",
    description: "Build and package reusable agent instructions for Zed.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 22 days ago",
    status: "Review",
    icon: RiStackLine,
  },
  {
    name: "Find skills",
    slug: "find-skills",
    description: "Discover installable skills for common development tasks.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 23 days ago",
    status: "Ready",
    icon: RiDatabase2Line,
  },
  {
    name: "Ponytail",
    slug: "ponytail",
    description: "A pragmatic coding style focused on simple, direct solutions.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 24 days ago",
    status: "Ready",
    icon: RiTerminalLine,
  },
  {
    name: "Ponytail review",
    slug: "ponytail-review",
    description: "A review pass dedicated to finding unnecessary complexity.",
    source: "Built-in",
    scope: "Global",
    updated: "Updated 25 days ago",
    status: "Review",
    icon: RiTerminalLine,
  },
  {
    name: "Rust testing",
    slug: "rust-testing",
    description: "Testing patterns for Rust unit, integration, and async code.",
    source: "Registry",
    scope: "Global",
    updated: "Updated 4 weeks ago",
    status: "Ready",
    icon: RiFileTextLine,
  },
  {
    name: "Systematic debugging",
    slug: "systematic-debugging-advanced",
    description: "A structured workflow for diagnosing complex regressions.",
    source: "Local",
    scope: "YssBI",
    updated: "Updated 29 days ago",
    status: "Syncing",
    icon: RiCommandLine,
  },
  {
    name: "Accessibility review",
    slug: "accessibility-review",
    description: "Review interactive interfaces for keyboard and screen reader use.",
    source: "Local",
    scope: "YssBI",
    updated: "Updated 1 month ago",
    status: "Review",
    icon: RiDatabase2Line,
  },
  {
    name: "IPC boundary design",
    slug: "ipc-boundary-design",
    description: "Keep frontend, command, application, and domain contracts clear.",
    source: "Local",
    scope: "YssBI",
    updated: "Updated 1 month ago",
    status: "Ready",
    icon: RiStackLine,
  },
  {
    name: "Release checklist",
    slug: "release-checklist",
    description: "Final validation steps for a safe and repeatable release.",
    source: "Local",
    scope: "YssBI",
    updated: "Updated 5 weeks ago",
    status: "Ready",
    icon: RiFileTextLine,
  },
  {
    name: "Documentation maintenance",
    slug: "documentation-maintenance",
    description: "Keep architecture and development documentation current.",
    source: "Local",
    scope: "YssBI",
    updated: "Updated 6 weeks ago",
    status: "Review",
    icon: RiDatabase2Line,
  },
];

const skillSets = [
  {
    name: "Reliable delivery",
    slug: "reliable-delivery",
    description: "A focused set for planning, implementing, and verifying changes.",
    skills: ["Rust patterns", "Test-driven development"],
    scope: "YssBI",
    updated: "Updated today",
    icon: RiStackLine,
  },
  {
    name: "Debugging toolkit",
    slug: "debugging-toolkit",
    description: "A practical set for isolating failures and reviewing regressions.",
    skills: ["Systematic debugging", "Frontend design"],
    scope: "Global",
    updated: "Updated yesterday",
    icon: RiCommandLine,
  },
];

const skillSubtitleOverrides: Record<string, string> = {
  brainstorming: "obra/superpowers",
};

function getSkillSubtitle(slug: string) {
  const override = skillSubtitleOverrides[slug];
  if (override) {
    return override;
  }

  const skill = skills.find((candidate) => candidate.slug === slug);
  if (skill) {
    return skill.source;
  }

  const skillSet = skillSets.find((candidate) => candidate.slug === slug);
  return skillSet?.scope ?? "Skill set";
}

type SkillListItem = {
  slug: string;
  name: string;
  description: string;
};

function SkillList({
  items,
  selectedSlugs,
  onToggle,
}: {
  items: SkillListItem[];
  selectedSlugs: ReadonlySet<string>;
  onToggle: (slug: string, checked?: boolean) => void;
}) {
  const keepCheckboxesVisible = selectedSlugs.size > 0;

  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {items.map((item) => {
        const isSelected = selectedSlugs.has(item.slug);

        return (
          <div
            key={item.slug}
            role="button"
            tabIndex={0}
            aria-pressed={isSelected}
            className="group/skill-row mx-4 grid min-h-14 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_minmax(0,2fr)_2rem] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50 lg:mx-6 lg:px-6"
            onClick={() => onToggle(item.slug)}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget) {
                return;
              }

              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onToggle(item.slug);
              }
            }}
          >
            <div className="flex size-6 items-center justify-center">
              <Checkbox
                aria-label={`Select ${item.name}`}
                checked={isSelected}
                className={cn(
                  "opacity-0 transition-opacity group-hover/skill-row:opacity-100 group-focus-within/skill-row:opacity-100",
                  keepCheckboxesVisible && "opacity-100",
                )}
                onClick={(event) => event.stopPropagation()}
                onCheckedChange={(checked) => onToggle(item.slug, checked === true)}
              />
            </div>
            <div className="flex min-w-0 items-baseline gap-2">
              <span className="min-w-0 truncate font-medium">{item.name}</span>
              <span className="max-w-32 shrink-0 truncate text-[0.65rem] text-muted-foreground">
                {getSkillSubtitle(item.slug)}
              </span>
            </div>
            <div className="min-w-0 truncate text-muted-foreground">{item.description}</div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={`Edit ${item.name}`}
              title={`Edit ${item.name}`}
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

export function SkillsPage() {
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

  const filteredSkills = useMemo(() => {
    if (!normalizedQuery) {
      return skills;
    }

    return skills.filter((skill) =>
      `${skill.name} ${skill.description} ${skill.source} ${skill.scope}`
        .toLowerCase()
        .includes(normalizedQuery),
    );
  }, [normalizedQuery]);

  const filteredSkillSets = useMemo(() => {
    if (!normalizedQuery) {
      return skillSets;
    }

    return skillSets.filter((skillSet) =>
      `${skillSet.name} ${skillSet.description} ${skillSet.skills.join(" ")} ${skillSet.scope}`
        .toLowerCase()
        .includes(normalizedQuery),
    );
  }, [normalizedQuery]);

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Skills</h1>
        <Button type="button" variant="outline" size="sm">
          <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
          Refresh
        </Button>
      </header>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border bg-background">
        <Tabs defaultValue="item" className="min-h-0 min-w-0 flex-1 gap-0">
          <div className="flex flex-col gap-3 p-4 lg:flex-row lg:items-center lg:justify-between lg:px-6">
            <div className="relative w-full max-w-md">
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
            <TabsList className="w-full lg:w-auto">
              <TabsTrigger value="item" className="flex-1 lg:flex-none">
                Item
              </TabsTrigger>
              <TabsTrigger value="set" className="flex-1 lg:flex-none">
                Set
              </TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value="item" className="flex min-h-0 min-w-0 flex-1 flex-col">
            {filteredSkills.length > 0 ? (
              <ScrollArea className="min-h-0 min-w-0 flex-1">
                <SkillList
                  items={filteredSkills}
                  selectedSlugs={selectedSlugs}
                  onToggle={toggleSelection}
                />
              </ScrollArea>
            ) : (
              <div className="px-4 py-10 text-center lg:px-6">
                <p className="font-medium">No matching items</p>
                <p className="mt-1 text-sm text-muted-foreground">Try a different search term.</p>
              </div>
            )}
          </TabsContent>

          <TabsContent value="set" className="flex min-h-0 min-w-0 flex-1 flex-col">
            {filteredSkillSets.length > 0 ? (
              <ScrollArea className="min-h-0 min-w-0 flex-1">
                <SkillList
                  items={filteredSkillSets}
                  selectedSlugs={selectedSlugs}
                  onToggle={toggleSelection}
                />
              </ScrollArea>
            ) : (
              <div className="px-4 py-10 text-center lg:px-6">
                <p className="font-medium">No matching sets</p>
                <p className="mt-1 text-sm text-muted-foreground">Try a different search term.</p>
              </div>
            )}
          </TabsContent>
        </Tabs>
      </div>
    </>
  );
}
