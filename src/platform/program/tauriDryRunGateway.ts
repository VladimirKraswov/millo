import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { DryRunGateway, SenderSnapshot } from "../../shared/dryRun";

export const tauriDryRunGateway: DryRunGateway = {
  snapshot: () => invoke<SenderSnapshot>("sender_snapshot"),
  start: (request) =>
    invoke<SenderSnapshot>("start_mock_dry_run", { request }),
  pause: () => invoke<SenderSnapshot>("pause_dry_run"),
  resume: () => invoke<SenderSnapshot>("resume_dry_run"),
  cancel: () => invoke<SenderSnapshot>("cancel_dry_run"),
  subscribe: async (listener) =>
    listen<SenderSnapshot>("dry-run-state", (event) => listener(event.payload)),
};

