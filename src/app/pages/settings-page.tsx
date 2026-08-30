import { useState } from "react";
import { Link } from "react-router";
import { RiFolderOpenLine } from "@remixicon/react";
import { useTheme } from "next-themes";
import { toast } from "sonner";

import { selectDirectory } from "@/app/services/directory-picker";

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
  const [centralSkillsPath, setCentralSkillsPath] = useState("D:/Projects/shared-skills");
  const [isSelectingCentralSkillsPath, setIsSelectingCentralSkillsPath] = useState(false);

  const handleSelectCentralSkillsPath = async () => {
    setIsSelectingCentralSkillsPath(true);

    try {
      const selectedPath = await selectDirectory("Select central skills repository");

      if (selectedPath) {
        setCentralSkillsPath(selectedPath);
      }
    } catch {
      toast.error("Unable to open the folder picker.");
    } finally {
      setIsSelectingCentralSkillsPath(false);
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
            <Card>
              <CardContent className="flex flex-col gap-5">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex min-w-0 flex-col gap-1">
                    <span className="font-medium">Central skills repository</span>
                    <span
                      aria-live="polite"
                      className="truncate text-sm text-muted-foreground"
                      title={centralSkillsPath}
                    >
                      {centralSkillsPath}
                    </span>
                  </div>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    aria-label="Choose central skills repository"
                    onClick={() => void handleSelectCentralSkillsPath()}
                    disabled={isSelectingCentralSkillsPath}
                  >
                    <RiFolderOpenLine aria-hidden="true" data-icon="inline-start" />
                    Choose
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
                      Choose the language for the interface.
                    </span>
                  </div>
                  <Select defaultValue="zh-CN">
                    <SelectTrigger
                      className="w-full sm:w-48"
                      size="sm"
                      aria-label="Language"
                    >
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
