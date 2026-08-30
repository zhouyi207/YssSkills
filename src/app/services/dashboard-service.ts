import { invokeCommand } from "./ipc-client";
import { dashboardOverviewDtoSchema, type DashboardOverviewDto } from "@/shared/types/dashboard";

export const dashboardService = {
  getDashboardOverview(): Promise<DashboardOverviewDto> {
    return invokeCommand("get_dashboard_overview", dashboardOverviewDtoSchema);
  },
};
