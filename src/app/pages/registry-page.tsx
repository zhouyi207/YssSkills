import { useMemo, useState } from "react";
import { RiRefreshLine, RiSearchLine } from "@remixicon/react";

import { skills } from "./skills-page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

const registrySkills = [
  {
    slug: "code-review",
    name: "Code review",
    publisher: "obra/superpowers",
    category: "Engineering",
    installs: "4.2k",
  },
  {
    slug: "receiving-code-review",
    name: "Receiving code review",
    publisher: "obra/superpowers",
    category: "Engineering",
    installs: "2.8k",
  },
  {
    slug: "software-dev-process",
    name: "Software development process",
    publisher: "YssSkills",
    category: "Workflow",
    installs: "1.9k",
  },
  {
    slug: "tiptap",
    name: "Tiptap",
    publisher: "ueberdosis/tiptap",
    category: "Editor",
    installs: "1.4k",
  },
];

type RegistrySkill = (typeof registrySkills)[number];

const installedSkillSlugs = new Set(skills.map((skill) => skill.slug));

function RegistrySkillList({ items }: { items: RegistrySkill[] }) {
  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {items.map((skill) => {
        const isInstalled = installedSkillSlugs.has(skill.slug);

        return (
          <div
            key={skill.slug}
            role="listitem"
            className={cn(
              "mx-4 grid min-h-14 min-w-0 grid-cols-[minmax(0,2fr)_minmax(0,1fr)_auto] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50 lg:mx-6 lg:px-6",
              isInstalled && "border-primary/40 bg-muted/40",
            )}
          >
            <div className="flex min-w-0 items-baseline gap-2">
              <h3 className="min-w-0 truncate font-medium">{skill.name}</h3>
              <span className="max-w-36 shrink-0 truncate text-[0.65rem] text-muted-foreground">
                {skill.publisher}
              </span>
              {isInstalled ? <Badge variant="secondary">Installed</Badge> : null}
            </div>
            <div className="min-w-0 truncate text-muted-foreground">{skill.category}</div>
            <div className="shrink-0 text-right text-muted-foreground">{skill.installs}</div>
          </div>
        );
      })}
    </div>
  );
}

export function RegistryPage() {
  const [query, setQuery] = useState("");
  const [submittedQuery, setSubmittedQuery] = useState("");

  const filteredSkills = useMemo(() => {
    const normalizedQuery = submittedQuery.trim().toLowerCase();

    if (!normalizedQuery) {
      return registrySkills;
    }

    return registrySkills.filter((skill) =>
      `${skill.name} ${skill.publisher} ${skill.category} ${skill.installs}`
        .toLowerCase()
        .includes(normalizedQuery),
    );
  }, [submittedQuery]);

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Registry</h1>
        <Button type="button" variant="outline" size="sm">
          <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
          Refresh
        </Button>
      </header>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border bg-background">
        <form
          className="flex w-full items-center gap-2 p-4 lg:px-6"
          onSubmit={(event) => {
            event.preventDefault();
            setSubmittedQuery(query);
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
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder="Search registry skills"
              className="pl-8"
            />
          </div>
          <Button type="submit">
            <RiSearchLine aria-hidden="true" data-icon="inline-start" />
            Search
          </Button>
        </form>

        {filteredSkills.length > 0 ? (
          <ScrollArea className="min-h-0 min-w-0 flex-1">
            <RegistrySkillList items={filteredSkills} />
          </ScrollArea>
        ) : (
          <div className="px-4 py-10 text-center lg:px-6" aria-live="polite">
            <p className="font-medium">No matching skills</p>
            <p className="mt-1 text-sm text-muted-foreground">
              Try a broader search term or browse the registry again.
            </p>
          </div>
        )}
      </div>
    </>
  );
}
