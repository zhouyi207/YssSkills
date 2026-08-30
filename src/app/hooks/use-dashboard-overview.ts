import { dashboardService } from "@/app/services/dashboard-service";
import { useServiceResource } from "./use-service-resource";

const loadDashboardOverview = () => dashboardService.getDashboardOverview();

export function useDashboardOverview() {
  return useServiceResource(loadDashboardOverview);
}
