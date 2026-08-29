import { Link } from "react-router";

import { PageHeader } from "@/app/pages/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        eyebrow="Preferences"
        title="Settings"
        description="Set the defaults that keep skill discovery and deployment predictable."
        action={<Badge variant="secondary">Local only</Badge>}
      />

      <div className="p-4 lg:p-6">
        <Tabs defaultValue="general" className="w-full">
          <TabsList>
            <TabsTrigger value="general">General</TabsTrigger>
            <TabsTrigger value="appearance">Appearance</TabsTrigger>
          </TabsList>

          <TabsContent value="general" className="mt-4">
            <Card>
              <CardHeader>
                <CardTitle>General preferences</CardTitle>
                <CardDescription>
                  Defaults used when scanning, importing, and displaying skills.
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-5">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex flex-col gap-1">
                    <span className="font-medium">Default workspace</span>
                    <span className="text-sm text-muted-foreground">
                      Where new skills are proposed first.
                    </span>
                  </div>
                  <Select defaultValue="global">
                    <SelectTrigger
                      className="w-full sm:w-48"
                      size="sm"
                      aria-label="Default workspace"
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="global">Global</SelectItem>
                        <SelectItem value="project">YssBI project</SelectItem>
                        <SelectItem value="linked">Linked workspace</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </div>

                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex flex-col gap-1">
                    <span className="font-medium">Install behavior</span>
                    <span className="text-sm text-muted-foreground">
                      Keep changes staged until you review the operation.
                    </span>
                  </div>
                  <Select defaultValue="review">
                    <SelectTrigger
                      className="w-full sm:w-48"
                      size="sm"
                      aria-label="Install behavior"
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="review">Review first</SelectItem>
                        <SelectItem value="copy">Copy immediately</SelectItem>
                        <SelectItem value="link">Link immediately</SelectItem>
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </div>
              </CardContent>
              <CardFooter className="justify-between">
                <span className="text-xs text-muted-foreground">
                  Preview only; persistence is not connected.
                </span>
                <Badge variant="outline">Not persisted</Badge>
              </CardFooter>
            </Card>
          </TabsContent>

          <TabsContent value="appearance" className="mt-4">
            <Card>
              <CardHeader>
                <CardTitle>Appearance</CardTitle>
                <CardDescription>
                  Tune the density and language of the local console.
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-5">
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex flex-col gap-1">
                    <span className="font-medium">Language preview</span>
                    <span className="text-sm text-muted-foreground">
                      Preview only; language switching will be wired to i18n later.
                    </span>
                  </div>
                  <Select defaultValue="zh-CN">
                    <SelectTrigger
                      className="w-full sm:w-48"
                      size="sm"
                      aria-label="Language preview"
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
                    <span className="font-medium">Interface density preview</span>
                    <span className="text-sm text-muted-foreground">
                      Preview only; density will be wired to the layout later.
                    </span>
                  </div>
                  <Select defaultValue="comfortable">
                    <SelectTrigger
                      className="w-full sm:w-48"
                      size="sm"
                      aria-label="Interface density preview"
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="comfortable">Comfortable</SelectItem>
                        <SelectItem value="compact">Compact</SelectItem>
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
