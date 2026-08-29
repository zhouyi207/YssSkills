import { useMemo, useState } from "react";
import { RiDownloadLine, RiExternalLinkLine, RiRefreshLine, RiSearchLine } from "@remixicon/react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { toast } from "sonner";

import { skills } from "./skills-page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

const registrySkills = [
  {
    slug: "code-review",
    name: "Code review",
    publisher: "obra/superpowers",
    installs: "4.2k",
  },
  {
    slug: "receiving-code-review-12",
    name: "Receiving code review",
    publisher: "obra/superpowers",
    installs: "2.8k",
  },
  {
    slug: "software-dev-process",
    name: "Software development process",
    publisher: "YssSkills",
    installs: "1.9k",
  },
  {
    slug: "tiptap",
    name: "Tiptap",
    publisher: "ueberdosis/tiptap",
    installs: "1.4k",
  },
];

type RegistrySkill = (typeof registrySkills)[number];

const defaultRegistryUrl = "https://www.google.com";

async function openRegistrySkillWindow(skill: RegistrySkill) {
  const label = `registry-${skill.slug}`;

  try {
    const existingWindow = await WebviewWindow.getByLabel(label);
    if (existingWindow) {
      await existingWindow.setFocus();
      return;
    }

    const registryWindow = new WebviewWindow(label, {
      title: skill.name,
      url: defaultRegistryUrl,
      width: 1000,
      height: 700,
      resizable: true,
    });

    void registryWindow
      .once("tauri://error", () => {
        toast.error("Unable to open registry page.");
      })
      .catch(() => {
        toast.error("Unable to open registry page.");
      });
  } catch {
    toast.error("Unable to open registry page.");
  }
}

function RegistrySkillList({
  items,
  selectedSlugs,
  installedSlugs,
  isInstalling,
  onToggle,
}: {
  items: RegistrySkill[];
  selectedSlugs: ReadonlySet<string>;
  installedSlugs: ReadonlySet<string>;
  isInstalling: boolean;
  onToggle: (slug: string, checked?: boolean) => void;
}) {
  const keepCheckboxesVisible = selectedSlugs.size > 0;

  return (
    <div role="list" className="flex min-w-0 flex-col gap-2 text-xs/relaxed">
      {items.map((skill) => {
        const isInstalled = installedSlugs.has(skill.slug);
        const isSelected = selectedSlugs.has(skill.slug);
        const isChecked = isInstalled || isSelected;
        const isDisabled = isInstalled || isInstalling;

        return (
          <div
            key={skill.slug}
            role="button"
            tabIndex={isDisabled ? -1 : 0}
            aria-pressed={isChecked}
            aria-disabled={isDisabled}
            className={cn(
              "group/registry-row mx-4 grid min-h-14 min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_auto_2rem] items-center gap-3 border px-4 py-2 outline-none transition-colors hover:bg-muted/50 focus-visible:bg-muted/50",
              isChecked && "border-primary/40 bg-muted/40",
            )}
            onClick={() => {
              if (!isDisabled) {
                onToggle(skill.slug);
              }
            }}
            onKeyDown={(event) => {
              if (event.target !== event.currentTarget || isDisabled) {
                return;
              }

              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onToggle(skill.slug);
              }
            }}
          >
            <div className="flex size-6 items-center justify-center">
              <Checkbox
                checked={isChecked}
                disabled={isDisabled}
                aria-label={
                  isInstalled ? `${skill.name} installed` : `Select ${skill.name} for installation`
                }
                className={cn(
                  "opacity-0 transition-opacity group-hover/registry-row:opacity-100 group-focus-within/registry-row:opacity-100",
                  (keepCheckboxesVisible || isInstalled) && "opacity-100",
                )}
                onClick={(event) => event.stopPropagation()}
                onCheckedChange={(checked) => onToggle(skill.slug, checked === true)}
              />
            </div>
            <div className="flex min-w-0 items-baseline gap-2">
              <h3 className="min-w-0 truncate font-medium">{skill.name}</h3>
              <span className="max-w-36 shrink-0 truncate text-[0.65rem] text-muted-foreground">
                {skill.publisher}
              </span>
            </div>
            <div className="shrink-0 justify-self-end">
              <Badge variant="secondary">{skill.installs}</Badge>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={`Open ${skill.name}`}
              title={`Open ${skill.name}`}
              className="justify-self-end"
              onClick={(event) => {
                event.stopPropagation();
                void openRegistrySkillWindow(skill);
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
  const [submittedQuery, setSubmittedQuery] = useState("");
  const [selectedSlugs, setSelectedSlugs] = useState<Set<string>>(() => new Set());
  const [installedSlugs, setInstalledSlugs] = useState<Set<string>>(
    () => new Set(skills.map((skill) => skill.slug)),
  );
  const [isInstalling, setIsInstalling] = useState(false);

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
    const normalizedQuery = submittedQuery.trim().toLowerCase();

    if (!normalizedQuery) {
      return registrySkills;
    }

    return registrySkills.filter((skill) =>
      `${skill.name} ${skill.publisher} ${skill.installs}`
        .toLowerCase()
        .includes(normalizedQuery),
    );
  }, [submittedQuery]);

  const handleInstallSkills = () => {
    const slugsToInstall = [...selectedSlugs].filter((slug) => !installedSlugs.has(slug));

    if (slugsToInstall.length === 0) {
      return;
    }

    setIsInstalling(true);

    const installation = new Promise<void>((resolve) => {
      window.setTimeout(resolve, 1600);
    });

    void toast.promise(installation, {
      loading: `Downloading ${slugsToInstall.length} skills...`,
      success: () => {
        setInstalledSlugs((current) => {
          const next = new Set(current);
          slugsToInstall.forEach((slug) => next.add(slug));
          return next;
        });
        setSelectedSlugs((current) => {
          const next = new Set(current);
          slugsToInstall.forEach((slug) => next.delete(slug));
          return next;
        });
        setIsInstalling(false);
        return `${slugsToInstall.length} skills installed.`;
      },
      error: () => {
        setIsInstalling(false);
        return "Unable to install skills.";
      },
    });
  };

  return (
    <>
      <header className="flex shrink-0 items-center justify-between">
        <h1 className="font-heading text-sm font-medium">Registry</h1>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={selectedSlugs.size === 0 || isInstalling}
            onClick={handleInstallSkills}
          >
            <RiDownloadLine aria-hidden="true" data-icon="inline-start" />
            Install
          </Button>
          <Button type="button" variant="outline" size="sm">
            <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
            Refresh
          </Button>
        </div>
      </header>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border bg-background">
        <form
          className="flex w-full items-center gap-2 p-4"
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
            <RegistrySkillList
              items={filteredSkills}
              selectedSlugs={selectedSlugs}
              installedSlugs={installedSlugs}
              isInstalling={isInstalling}
              onToggle={toggleSelection}
            />
          </ScrollArea>
        ) : (
          <div className="px-4 py-10 text-center" aria-live="polite">
            <p className="text-sm font-medium">No matching results</p>
            <p className="mt-1 text-sm text-muted-foreground">Try a different search term.</p>
          </div>
        )}
      </div>
    </>
  );
}
