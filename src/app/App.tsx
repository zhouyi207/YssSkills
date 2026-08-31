import "./App.css";
import { useEffect } from "react";
import { registryService } from "./services/registry-service";
import { AppRouter } from "./routes";

export default function App() {
  useEffect(() => {
    registryService.preloadRegistry();
  }, []);

  return <AppRouter />;
}
