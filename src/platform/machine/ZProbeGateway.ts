import type { ZProbeOutcome, ZProbeRequest } from "../../shared/machine";

export interface ZProbeGateway {
  run(request: ZProbeRequest): Promise<ZProbeOutcome>;
}
