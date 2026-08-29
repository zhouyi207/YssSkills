import { open, save } from "@tauri-apps/plugin-dialog";

const skillArchiveFilter = {
  name: "Skill archive",
  extensions: ["zip"],
};

export async function selectSkillFiles(): Promise<string[]> {
  const selection = await open({
    directory: false,
    multiple: true,
    title: "Import skills",
    filters: [skillArchiveFilter],
  });

  if (!selection) {
    return [];
  }

  return Array.isArray(selection) ? selection : [selection];
}

export async function selectSkillExportPath(): Promise<string | null> {
  return save({
    defaultPath: "skills.zip",
    title: "Export skills",
    filters: [skillArchiveFilter],
  });
}
