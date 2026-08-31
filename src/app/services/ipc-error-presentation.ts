import { ipcErrorSchema, type IpcError } from "@/shared/types/ipc";
import { getUnknownErrorReason } from "./ipc-client";

export type IpcErrorDetail = Readonly<{
  label: string;
  value: string;
}>;

const CONTEXT_LABELS: Readonly<Record<string, string>> = {
  actual: "Actual",
  candidates: "Candidates",
  cause: "Cause",
  causeMessage: "Cause",
  cleanupError: "Cleanup error",
  command: "Command",
  deploymentKey: "Deployment key",
  entity: "Entity",
  expected: "Expected",
  field: "Field",
  harnessId: "Harness ID",
  id: "ID",
  item: "Item",
  kind: "Kind",
  limit: "Limit",
  maximum: "Maximum",
  minimum: "Minimum",
  observed: "Observed",
  operation: "Operation",
  operationError: "Operation error",
  path: "Path",
  reason: "Reason",
  requested: "Requested",
  root: "Root",
  sourcePath: "Source path",
  status: "Status",
  targetPath: "Target path",
  workspaceId: "Workspace ID",
};

function contextLabel(key: string) {
  const knownLabel = CONTEXT_LABELS[key];
  if (knownLabel) {
    return knownLabel;
  }

  const words = key
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[._-]+/g, " ")
    .trim();
  if (!words) {
    return "Detail";
  }

  const label = words.charAt(0).toUpperCase() + words.slice(1);
  return label.replace(/\bId\b/g, "ID").replace(/\bUrl\b/g, "URL");
}

function retryAfterDetail(error: IpcError): IpcErrorDetail | null {
  if (!error.retryAfter) {
    return null;
  }
  if (error.retryAfter.kind === "delay") {
    const { seconds } = error.retryAfter;
    return {
      label: "Retry after",
      value: `${seconds} second${seconds === 1 ? "" : "s"}`,
    };
  }

  const date = new Date(error.retryAfter.epochMillis);
  return {
    label: "Retry after",
    value: Number.isNaN(date.getTime())
      ? `${error.retryAfter.epochMillis} ms since the Unix epoch`
      : date.toLocaleString(),
  };
}

export function getIpcErrorDetails(error: IpcError): IpcErrorDetail[] {
  const details = Object.entries(error.context ?? {}).map(([key, value]) => ({
    label: contextLabel(key),
    value: value || "(empty)",
  }));
  const retryAfter = retryAfterDetail(error);
  if (retryAfter) {
    details.push(retryAfter);
  }
  return details;
}

export function formatIpcError(error: IpcError): string {
  const details = getIpcErrorDetails(error).map(({ label, value }) => `${label}: ${value}`);
  return `${error.message} ${[...details, `Error code: ${error.code}`].join(" · ")}`;
}

export function formatUnknownError(error: unknown, fallbackMessage: string): string {
  const parsedIpcError = ipcErrorSchema.safeParse(error);
  if (parsedIpcError.success) {
    return formatIpcError(parsedIpcError.data);
  }

  const reason = getUnknownErrorReason(error);
  if (!reason || reason === fallbackMessage) {
    return fallbackMessage;
  }
  return `${fallbackMessage} Reason: ${reason}`;
}
