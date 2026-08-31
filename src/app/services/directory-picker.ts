import { open } from "@tauri-apps/plugin-dialog";

export async function selectDirectory(title: string, defaultPath?: string): Promise<string | null> {
  const selection = await open({
    directory: true,
    multiple: false,
    title,
    defaultPath,
  });

  return typeof selection === "string" ? selection : null;
}
