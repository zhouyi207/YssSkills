import { useCallback, useRef, useState } from "react";

import { isIpcError } from "@/app/services/ipc-client";
import { workspacesService } from "@/app/services/workspaces-service";
import type { IpcError } from "@/shared/types/ipc";
import type {
  AddDetectedAgentsResponseDto,
  CopyProjectAgentSkillsRequestDto,
  CopyProjectAgentSkillsResponseDto,
  DeleteAgentsResponseDto,
  DeleteProjectAgentsResponseDto,
  DetectAgentsResponseDto,
  SaveAgentRequestDto,
  SaveAgentResponseDto,
  WorkspaceObservationDto,
  WorkspaceReconcileOutcomeDto,
  WorkspaceSummaryDto,
} from "@/shared/types/workspaces";
import { unexpectedClientError, useServiceResource } from "./use-service-resource";

const loadWorkspaces = () => workspacesService.getWorkspacesOverview();

export function useWorkspaces() {
  const resource = useServiceResource(loadWorkspaces);
  const [observation, setObservation] = useState<WorkspaceObservationDto | null>(null);
  const [observationError, setObservationError] = useState<IpcError | null>(null);
  const [isObserving, setIsObserving] = useState(false);
  const [mutationError, setMutationError] = useState<IpcError | null>(null);
  const [isMutating, setIsMutating] = useState(false);
  const [detectionError, setDetectionError] = useState<IpcError | null>(null);
  const [isDetectingAgents, setIsDetectingAgents] = useState(false);
  const observationRequestId = useRef(0);

  const observe = useCallback(async (workspaceId: string) => {
    const requestId = ++observationRequestId.current;
    setObservationError(null);
    setIsObserving(true);

    try {
      const next = await workspacesService.observeWorkspace({ workspaceId });
      if (observationRequestId.current === requestId) {
        setObservation(next);
      }
      return next;
    } catch (caught: unknown) {
      if (observationRequestId.current === requestId) {
        setObservationError(isIpcError(caught) ? caught : unexpectedClientError(caught));
      }
      return null;
    } finally {
      if (observationRequestId.current === requestId) {
        setIsObserving(false);
      }
    }
  }, []);

  const createProject = useCallback(
    async (name: string, root: string): Promise<WorkspaceSummaryDto | null> => {
      setMutationError(null);
      setIsMutating(true);
      try {
        const created = await workspacesService.createWorkspace({
          name,
          kind: { kind: "project", root },
          deploymentMode: "copy",
        });
        await resource.refresh();
        return created;
      } catch (caught: unknown) {
        setMutationError(isIpcError(caught) ? caught : unexpectedClientError(caught));
        return null;
      } finally {
        setIsMutating(false);
      }
    },
    [resource.refresh],
  );

  const detectAgents = useCallback(async (): Promise<DetectAgentsResponseDto | null> => {
    setDetectionError(null);
    setIsDetectingAgents(true);
    try {
      return await workspacesService.detectAgents();
    } catch (caught: unknown) {
      setDetectionError(isIpcError(caught) ? caught : unexpectedClientError(caught));
      return null;
    } finally {
      setIsDetectingAgents(false);
    }
  }, []);

  const addDetectedAgents = useCallback(
    async (detectorIds: string[]): Promise<AddDetectedAgentsResponseDto | null> => {
      setMutationError(null);
      setIsMutating(true);
      try {
        const outcome = await workspacesService.addDetectedAgents({ detectorIds });
        await resource.refresh();
        return outcome;
      } catch (caught: unknown) {
        setMutationError(isIpcError(caught) ? caught : unexpectedClientError(caught));
        return null;
      } finally {
        setIsMutating(false);
      }
    },
    [resource.refresh],
  );

  const deleteAgents = useCallback(
    async (agentIds: string[]): Promise<DeleteAgentsResponseDto | null> => {
      setMutationError(null);
      setIsMutating(true);
      try {
        const outcome = await workspacesService.deleteAgents({ agentIds });
        await resource.refresh();
        return outcome;
      } catch (caught: unknown) {
        setMutationError(isIpcError(caught) ? caught : unexpectedClientError(caught));
        await resource.refresh();
        return null;
      } finally {
        setIsMutating(false);
      }
    },
    [resource.refresh],
  );

  const copyProjectAgentSkills = useCallback(
    async (
      request: CopyProjectAgentSkillsRequestDto,
    ): Promise<CopyProjectAgentSkillsResponseDto | null> => {
      setMutationError(null);
      setIsMutating(true);
      try {
        const outcome = await workspacesService.copyProjectAgentSkills(request);
        await observe(request.workspaceId);
        return outcome;
      } catch (caught: unknown) {
        setMutationError(isIpcError(caught) ? caught : unexpectedClientError(caught));
        return null;
      } finally {
        setIsMutating(false);
      }
    },
    [observe],
  );

  const deleteProjectAgents = useCallback(
    async (
      workspaceId: string,
      agentIds: string[],
    ): Promise<DeleteProjectAgentsResponseDto | null> => {
      setMutationError(null);
      setIsMutating(true);
      try {
        const outcome = await workspacesService.deleteProjectAgents({ workspaceId, agentIds });
        await resource.refresh();
        await observe(workspaceId);
        return outcome;
      } catch (caught: unknown) {
        setMutationError(isIpcError(caught) ? caught : unexpectedClientError(caught));
        return null;
      } finally {
        setIsMutating(false);
      }
    },
    [observe, resource.refresh],
  );

  const saveAgent = useCallback(
    async (request: SaveAgentRequestDto): Promise<SaveAgentResponseDto | null> => {
      setMutationError(null);
      setIsMutating(true);
      try {
        const outcome = await workspacesService.saveAgent(request);
        await resource.refresh();
        return outcome;
      } catch (caught: unknown) {
        setMutationError(isIpcError(caught) ? caught : unexpectedClientError(caught));
        await resource.refresh();
        return null;
      } finally {
        setIsMutating(false);
      }
    },
    [resource.refresh],
  );

  const reconcile = useCallback(
    async (workspaceId: string): Promise<WorkspaceReconcileOutcomeDto | null> => {
      setMutationError(null);
      setIsMutating(true);
      try {
        const outcome = await workspacesService.reconcileWorkspace({ workspaceId });
        void (async () => {
          await resource.refresh();
          await observe(workspaceId);
        })();
        return outcome;
      } catch (caught: unknown) {
        setMutationError(isIpcError(caught) ? caught : unexpectedClientError(caught));
        return null;
      } finally {
        setIsMutating(false);
      }
    },
    [observe, resource.refresh],
  );

  const clearMutationError = useCallback(() => setMutationError(null), []);

  return {
    ...resource,
    overview: resource.data,
    observation,
    observationError,
    isObserving,
    mutationError,
    isMutating,
    detectionError,
    isDetectingAgents,
    observe,
    createProject,
    detectAgents,
    addDetectedAgents,
    deleteAgents,
    copyProjectAgentSkills,
    deleteProjectAgents,
    saveAgent,
    reconcile,
    clearMutationError,
  };
}
