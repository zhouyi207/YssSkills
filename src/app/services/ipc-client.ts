import { invoke } from "@tauri-apps/api/core";
import type { ZodType } from "zod";

import { ipcErrorSchema, type IpcError } from "@/shared/types/ipc";

type InvokeArguments = Record<string, unknown>;

function createClientError(code: string, message: string, command: string): IpcError {
  return {
    code,
    message,
    retryable: false,
    context: { command },
  };
}

export function isIpcError(value: unknown): value is IpcError {
  return ipcErrorSchema.safeParse(value).success;
}

export function normalizeIpcError(rejection: unknown, command: string): IpcError {
  const result = ipcErrorSchema.safeParse(rejection);

  if (result.success) {
    return result.data;
  }

  return createClientError("ipc.invoke_failed", "The application request failed.", command);
}

export async function invokeCommand<Response>(
  command: string,
  responseSchema: ZodType<Response>,
  args?: InvokeArguments,
): Promise<Response> {
  let response: unknown;

  try {
    response =
      args === undefined ? await invoke<unknown>(command) : await invoke<unknown>(command, args);
  } catch (rejection: unknown) {
    throw normalizeIpcError(rejection, command);
  }

  const result = responseSchema.safeParse(response);

  if (!result.success) {
    throw createClientError(
      "ipc.invalid_response",
      "The application returned an invalid response.",
      command,
    );
  }

  return result.data;
}
