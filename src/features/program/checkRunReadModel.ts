import type { SenderSnapshot } from "../../shared/dryRun";
import { isSenderActive } from "./senderStateModel";

export interface CheckRunContext {
  readonly gatewayAvailable: boolean;
  readonly loading: boolean;
  readonly programLoaded: boolean;
  readonly serialAvailable: boolean;
}

export const canStartCheckRun = (
  sender: SenderSnapshot,
  context: CheckRunContext,
): boolean =>
  context.gatewayAvailable &&
  context.serialAvailable &&
  context.programLoaded &&
  !context.loading &&
  !isSenderActive(sender.state);
