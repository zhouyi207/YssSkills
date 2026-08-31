import { useCallback, useEffect, useRef, useState } from "react";

import { getUnknownErrorReason, isIpcError } from "@/app/services/ipc-client";
import type { IpcError } from "@/shared/types/ipc";

export type ServiceResource<T> = {
  data: T | null;
  error: IpcError | null;
  isLoading: boolean;
  isRefreshing: boolean;
  refresh: () => Promise<T | null>;
};

export function unexpectedClientError(cause?: unknown): IpcError {
  const reason = getUnknownErrorReason(cause);
  return {
    code: "ui.unexpected_error",
    message: "An unexpected application error occurred.",
    retryable: false,
    ...(reason ? { context: { reason } } : {}),
  };
}

export function useServiceResource<T>(loader: () => Promise<T>): ServiceResource<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<IpcError | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const requestId = useRef(0);
  const hasData = useRef(false);

  const refresh = useCallback(async () => {
    const currentRequest = ++requestId.current;
    if (hasData.current) {
      setIsRefreshing(true);
    } else {
      setIsLoading(true);
    }
    setError(null);

    try {
      const next = await loader();
      if (requestId.current !== currentRequest) {
        return null;
      }
      hasData.current = true;
      setData(next);
      return next;
    } catch (caught: unknown) {
      if (requestId.current !== currentRequest) {
        return null;
      }
      setError(isIpcError(caught) ? caught : unexpectedClientError(caught));
      return null;
    } finally {
      if (requestId.current === currentRequest) {
        setIsLoading(false);
        setIsRefreshing(false);
      }
    }
  }, [loader]);

  useEffect(() => {
    void refresh();
    return () => {
      requestId.current += 1;
    };
  }, [refresh]);

  return { data, error, isLoading, isRefreshing, refresh };
}
