import type {
  JogPadStepOutcome,
  JogPadStepRequest,
} from "../../shared/machine";

export interface MachineCommandGateway {
  jogPadStep(request: JogPadStepRequest): Promise<JogPadStepOutcome>;
}
