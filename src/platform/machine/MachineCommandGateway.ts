import type {
  ContinuousJogReceipt,
  ContinuousJogRequest,
  ControllerSnapshot,
  JogPadStepOutcome,
  JogPadStepRequest,
} from "../../shared/machine";

export interface MachineCommandGateway {
  jogPadStep(request: JogPadStepRequest): Promise<JogPadStepOutcome>;
  startContinuousJog(request: ContinuousJogRequest): Promise<ContinuousJogReceipt>;
  cancelJog(): Promise<ControllerSnapshot>;
}
