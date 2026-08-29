import { open } from "@tauri-apps/plugin-dialog";

export async function selectDirectory(title: string): Promise<string | null> {
  const selection = await open({
    directory: true,
    multiple: false,
    title,
  });

  return typeof selection === "string" ? selection : null;
}
