import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { FeatureErrorBoundary } from "./components/FeatureErrorBoundary";
import { ApplicationRecovery } from "./app/workspace/ApplicationRecovery";
import { DialogHost } from "./components/DialogSurface";
import "./styles.css";
import "./app/workspace/workspace.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <FeatureErrorBoundary name="Приложение" fallback={<ApplicationRecovery />}>
      <DialogHost>
        <App />
      </DialogHost>
    </FeatureErrorBoundary>
  </StrictMode>,
);
