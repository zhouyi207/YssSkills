import { createHashRouter, RouterProvider } from "react-router";

import { appRouteConfig } from "./route-config";

export const router = createHashRouter(appRouteConfig);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
