import { describe, expect, it } from "vitest";

import { formatIpcError, formatUnknownError, getIpcErrorDetails } from "./ipc-error-presentation";
import type { IpcError } from "@/shared/types/ipc";

describe("IPC error presentation", () => {
  const validationError: IpcError = {
    code: "request.invalid",
    message: "One or more request fields are invalid.",
    retryable: false,
    context: {
      field: "agentRoot",
      reason: "must not overlap the central catalog or a workspace root",
    },
  };

  it("keeps every structured context value visible", () => {
    expect(getIpcErrorDetails(validationError)).toEqual([
      { label: "Field", value: "agentRoot" },
      {
        label: "Reason",
        value: "must not overlap the central catalog or a workspace root",
      },
    ]);
    expect(formatIpcError(validationError)).toBe(
      "One or more request fields are invalid. Field: agentRoot · Reason: must not overlap the central catalog or a workspace root · Error code: request.invalid",
    );
  });

  it("keeps an unknown failure's available reason instead of replacing it", () => {
    expect(formatUnknownError(new Error("Access is denied."), "Unable to open the folder.")).toBe(
      "Unable to open the folder. Reason: Access is denied.",
    );
  });
});
