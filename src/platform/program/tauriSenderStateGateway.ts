import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { SenderSnapshot, SenderStateGateway } from "../../shared/dryRun";

export const tauriSenderStateGateway: SenderStateGateway = {
  snapshot: () => invoke<SenderSnapshot>("sender_snapshot"),
  subscribe: async (listener) =>
    listen<SenderSnapshot>("sender-state", (event) => listener(event.payload)),
};
