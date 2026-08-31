import { useCallback, useEffect, useRef, useState } from "react";

import { isIpcError } from "@/app/services/ipc-client";
import { registryService } from "@/app/services/registry-service";
import type { IpcError } from "@/shared/types/ipc";
import type { RegistryResultDto } from "@/shared/types/registry";
import { unexpectedClientError } from "./use-service-resource";

const DEFAULT_LIMIT = 100;

type RegistryRequest = { mode: "leaderboard" } | { mode: "search"; query: string };

export function useRegistry() {
  const [data, setData] = useState<RegistryResultDto | null>(null);
  const [error, setError] = useState<IpcError | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const requestId = useRef(0);
  const dataRef = useRef<RegistryResultDto | null>(null);
  const lastAttemptRef = useRef<RegistryRequest>({ mode: "leaderboard" });

  const execute = useCallback(async (request: RegistryRequest) => {
    lastAttemptRef.current = request;
    const currentRequest = ++requestId.current;
    if (dataRef.current) {
      setIsRefreshing(true);
    } else {
      setIsLoading(true);
    }
    setError(null);

    try {
      const result =
        request.mode === "search"
          ? await registryService.searchRegistry({ query: request.query, limit: DEFAULT_LIMIT })
          : await registryService.getRegistryLeaderboard({ leaderboard: "allTime" });
      if (requestId.current === currentRequest) {
        dataRef.current = result;
        setData(result);
      }
      return result;
    } catch (caught: unknown) {
      if (requestId.current === currentRequest) {
        setError(isIpcError(caught) ? caught : unexpectedClientError(caught));
      }
      return null;
    } finally {
      if (requestId.current === currentRequest) {
        setIsLoading(false);
        setIsRefreshing(false);
      }
    }
  }, []);

  const loadLeaderboard = useCallback(() => execute({ mode: "leaderboard" }), [execute]);

  const search = useCallback(
    async (rawQuery: string) => {
      const query = rawQuery.trim();
      if (!query) {
        return loadLeaderboard();
      }
      if (query.length < 2) {
        requestId.current += 1;
        setIsLoading(false);
        setIsRefreshing(false);
        setError({
          code: "registry.invalid_query",
          message: "Enter at least two characters to search the registry.",
          retryable: false,
          context: { minimum: "2" },
        });
        return null;
      }
      return execute({ mode: "search", query });
    },
    [execute, loadLeaderboard],
  );

  const refresh = useCallback(() => {
    const current = dataRef.current;
    if (current?.mode === "search") {
      return execute({ mode: "search", query: current.query });
    }
    return loadLeaderboard();
  }, [execute, loadLeaderboard]);

  const retry = useCallback(() => execute(lastAttemptRef.current), [execute]);

  useEffect(() => {
    void loadLeaderboard();
    return () => {
      requestId.current += 1;
    };
  }, [loadLeaderboard]);

  return { data, error, isLoading, isRefreshing, search, refresh, retry };
}
