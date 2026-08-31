import { useEffect, useState } from "react";
import { Link } from "react-router";
import { RiFolderOpenLine, RiRefreshLine } from "@remixicon/react";
import { useTheme } from "next-themes";
import { toast } from "sonner";

import { useAppSettings } from "@/app/hooks/use-app-settings";
import { selectDirectory } from "@/app/services/directory-picker";
import { formatIpcError, formatUnknownError } from "@/app/services/ipc-error-presentation";
import { isIpcError } from "@/app/services/ipc-client";
import { skillsService } from "@/app/services/skills-service";
import { IpcErrorDetails } from "@/components/ipc-error-details";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

export function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const {
    settings,
    error,
    isLoading,
    isRefreshing,
    refresh,
    updateError,
    isUpdating,
    updateCatalogRoot,
  } = useAppSettings();
  const [updatedCatalogRootDisplay, setUpdatedCatalogRootDisplay] = useState<string | null>(null);
  const [isSelectingCentralSkillsPath, setIsSelectingCentralSkillsPath] = useState(false);
  const [isRebuildingSkillIndex, setIsRebuildingSkillIndex] = useState(false);

  const catalogRootDisplay = updatedCatalogRootDisplay ?? settings?.catalogRoot.display ?? "";
  const isChoosingCatalogRoot = isSelectingCentralSkillsPath || isUpdating;

  useEffect(() => {
    if (updateError) {
      toast.error(formatIpcError(updateError));
    }
  }, [updateError]);

  const handleRetrySettings = async () => {
    const refreshedSettings = await refresh();
    if (refreshedSettings !== null) {
      setUpdatedCatalogRootDisplay(refreshedSettings.catalogRoot.display);
    }
  };

  const handleSelectCentralSkillsPath = async () => {
    setIsSelectingCentralSkillsPath(true);

    try {
      const selectedPath = await selectDirectory("Select central skills repository");

      if (selectedPath === null) {
        return;
      }

      const updatedSettings = await updateCatalogRoot(selectedPath);
      if (updatedSettings !== null) {
        setUpdatedCatalogRootDisplay(updatedSettings.catalogRoot.display);
      }
    } catch (caught: unknown) {
      toast.error(formatUnknownError(caught, "Unable to open the folder picker."));
    } finally {
      setIsSelectingCentralSkillsPath(false);
    }
  };

  const handleRebuildSkillIndex = async () => {
    setIsRebuildingSkillIndex(true);
    try {
      const outcome = await skillsService.rebuildCatalogIndex();
      toast.success(
        `Skill index rebuilt: ${outcome.inserted} added, ${outcome.updated} updated, ${outcome.removed} removed, ${outcome.invalid} invalid.`,
      );
    } catch (caught: unknown) {
      toast.error(
        isIpcError(caught)
          ? formatIpcError(caught)
          : formatUnknownError(caught, "Unable to rebuild the Skill index."),
      );
    } finally {
      setIsRebuildingSkillIndex(false);
    }
  };

  return (
    <div className="flex flex-1 flex-col">
      <div className="p-4 lg:p-6">
        <Tabs defaultValue="general" className="w-full">
          <TabsList>
            <TabsTrigger value="general">General</TabsTrigger>
            <TabsTrigger value="appearance">Appearance</TabsTrigger>
          </TabsList>

          <TabsContent value="general" className="mt-4">
            <Card aria-busy={isLoading || isRefreshing || isUpdating}>
              <CardContent className="flex flex-col gap-5">
                {isLoading && settings === null ? (
                  <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                    <div className="flex min-w-0 flex-col gap-1">
                      <span className="font-medium">Central skills repository</span>
                      <span aria-live="polite" className="truncate text-sm text-muted-foreground">
                        Loading settings...
                      </span>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      aria-label="Choose central skills repository"
                      disabled
                    >
                      <RiFolderOpenLine aria-hidden="true" data-icon="inline-start" />
                      Choose
                    </Button>
                  </div>
                ) : error ? (
                  <div
                    className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
                    role="alert"
                  >
                    <div className="flex min-w-0 flex-col gap-1">
                      <span className="font-medium">Central skills repository</span>
                      <IpcErrorDetails error={error} compact />
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={isLoading || isRefreshing}
                      onClick={() => void handleRetrySettings()}
                    >
                      <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                      Retry
                    </Button>
                  </div>
                ) : settings ? (
                  <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                    <div className="flex min-w-0 flex-col gap-1">
                      <span className="font-medium">Central skills repository</span>
                      <span
                        aria-live="polite"
                        className="truncate text-sm text-muted-foreground"
                        title={catalogRootDisplay}
                      >
                        {catalogRootDisplay}
                      </span>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      aria-label="Choose central skills repository"
                      onClick={() => void handleSelectCentralSkillsPath()}
                      disabled={isChoosingCatalogRoot}
                    >
                      <RiFolderOpenLine aria-hidden="true" data-icon="inline-start" />
                      Choose
                    </Button>
                  </div>
                ) : (
                  <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                    <div className="flex min-w-0 flex-col gap-1">
                      <span className="font-medium">Central skills repository</span>
                      <span className="truncate text-sm text-muted-foreground">
                        Settings are unavailable.
                      </span>
                    </div>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => void handleRetrySettings()}
                    >
                      <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                      Retry
                    </Button>
                  </div>
                )}

                <div className="flex flex-col gap-2 border-t pt-5 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex min-w-0 flex-col gap-1">
                    <span className="font-medium">Skill index</span>
                    <span className="text-sm text-muted-foreground">
                      Rebuild the disposable index from the central Skills directory.
                    </span>
                  </div>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={isRebuildingSkillIndex}
                    onClick={() => void handleRebuildSkillIndex()}
                  >
                    <RiRefreshLine aria-hidden="true" data-icon="inline-start" />
                    {isRebuildingSkillIndex ? "Rebuilding..." : "Rebuild"}
                  </Button>
                </div>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="appearance" className="mt-4">
            <Card>
              <CardContent className="flex flex-col gap-5">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex flex-col gap-1">
                    <span className="font-medium">Language</span>
                    <span className="text-sm text-muted-foreground">
                      Language switching is not available yet.
                    </span>
                  </div>
                  <Select defaultValue="zh-CN" disabled>
                    <SelectTrigger className="w-full sm:w-48" size="sm" aria-label="Language">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="zh-CN">简体中文</SelectItem>
                        <SelectItem value="en-US">English</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </div>

                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex flex-col gap-1">
                    <span className="font-medium">Theme</span>
                    <span className="text-sm text-muted-foreground">
                      Follow the system theme or choose dark or light.
                    </span>
                  </div>
                  <Select value={theme ?? "system"} onValueChange={setTheme}>
                    <SelectTrigger className="w-full sm:w-48" size="sm" aria-label="Theme">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="system">System</SelectItem>
                        <SelectItem value="dark">Dark</SelectItem>
                        <SelectItem value="light">Light</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </div>
              </CardContent>
              <CardFooter className="justify-end">
                <Button variant="outline" asChild>
                  <Link to="/dashboard">Back to dashboard</Link>
                </Button>
              </CardFooter>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
