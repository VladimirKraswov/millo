import type { MachineCommandGateway } from "../../platform/machine/MachineCommandGateway";
import type {
  ContinuousJogReceipt,
  JogAxis,
  JogPadStepOutcome,
  OperatorConfirmation,
} from "../../shared/machine";

export const MIN_JOG_DISTANCE_MM = 0.01;
export const MAX_JOG_DISTANCE_MM = 100_000;
export const MIN_JOG_FEED_MM_PER_MIN = 10;
export const MAX_JOG_FEED_MM_PER_MIN = 100_000;

export const JOG_MOTION_PROFILE_IDS = ["precision", "position", "traverse"] as const;

export type JogMotionProfileId = (typeof JOG_MOTION_PROFILE_IDS)[number];
export interface JogMotionProfile {
  readonly id: JogMotionProfileId;
  readonly label: string;
  readonly distanceMm: number;
  readonly feedMmPerMin: number;
}

export const jogMotionProfiles = (
  maxDistanceMm: number,
  maxFeedMmPerMin: number,
): readonly JogMotionProfile[] => [
  {
    id: "precision",
    label: "Точно",
    distanceMm: Math.min(0.1, maxDistanceMm),
    feedMmPerMin: Math.min(100, maxFeedMmPerMin),
  },
  {
    id: "position",
    label: "Позиция",
    distanceMm: Math.min(1, maxDistanceMm),
    feedMmPerMin: Math.min(300, maxFeedMmPerMin),
  },
  {
    id: "traverse",
    label: "Быстро",
    distanceMm: Math.min(maxDistanceMm, Math.max(10, maxDistanceMm * 0.1)),
    feedMmPerMin: Math.max(
      MIN_JOG_FEED_MM_PER_MIN,
      Math.min(maxFeedMmPerMin, maxFeedMmPerMin * 0.8),
    ),
  },
];
export type JogDirection = -1 | 1;

export const jogOperatorConfirmation = (
  ready: boolean,
): OperatorConfirmation => ({
  spindleOff: ready,
  toolClear: ready,
  powerControlReachable: ready,
});

export class JogPadInteractor {
  private inFlight = false;
  private continuousSequence = 0;
  private continuousStarting = false;
  private continuousActive = false;

  constructor(private readonly gateway: MachineCommandGateway) {}

  async move(
    confirmation: OperatorConfirmation,
    axis: JogAxis,
    direction: JogDirection,
    distanceMm: number,
    feedMmPerMin: number,
  ): Promise<JogPadStepOutcome> {
    if (this.inFlight) {
      throw new Error("jog pad command is already in progress");
    }
    if (
      !Number.isFinite(distanceMm) ||
      distanceMm < MIN_JOG_DISTANCE_MM ||
      distanceMm > MAX_JOG_DISTANCE_MM
    ) {
      throw new Error(`jog distance must be ${MIN_JOG_DISTANCE_MM}..${MAX_JOG_DISTANCE_MM} mm`);
    }
    if (
      !Number.isFinite(feedMmPerMin) ||
      feedMmPerMin < MIN_JOG_FEED_MM_PER_MIN ||
      feedMmPerMin > MAX_JOG_FEED_MM_PER_MIN
    ) {
      throw new Error(
        `jog feed must be ${MIN_JOG_FEED_MM_PER_MIN}..${MAX_JOG_FEED_MM_PER_MIN} mm/min`,
      );
    }

    this.inFlight = true;
    try {
      return await this.gateway.jogPadStep({
        confirmation,
        axis,
        distanceMm: direction * distanceMm,
        feedMmPerMin,
      });
    } finally {
      this.inFlight = false;
    }
  }

  async startContinuous(
    confirmation: OperatorConfirmation,
    axis: JogAxis,
    direction: JogDirection,
    feedMmPerMin: number,
  ): Promise<ContinuousJogReceipt> {
    if (this.inFlight || this.continuousStarting || this.continuousActive) {
      throw new Error("jog command is already in progress");
    }
    if (
      !Number.isFinite(feedMmPerMin) ||
      feedMmPerMin < MIN_JOG_FEED_MM_PER_MIN ||
      feedMmPerMin > MAX_JOG_FEED_MM_PER_MIN
    ) {
      throw new Error(
        `jog feed must be ${MIN_JOG_FEED_MM_PER_MIN}..${MAX_JOG_FEED_MM_PER_MIN} mm/min`,
      );
    }

    const sequence = ++this.continuousSequence;
    this.continuousStarting = true;
    try {
      const receipt = await this.gateway.startContinuousJog({
        confirmation,
        axis,
        direction,
        feedMmPerMin,
      });
      if (sequence !== this.continuousSequence) {
        await this.cancelAcceptedContinuousJog();
      } else {
        this.continuousActive = true;
      }
      return receipt;
    } finally {
      this.continuousStarting = false;
    }
  }

  async stopContinuous(): Promise<void> {
    ++this.continuousSequence;
    if (this.continuousStarting || !this.continuousActive) return;
    await this.cancelAcceptedContinuousJog();
  }

  isContinuousBusy(): boolean {
    return this.continuousStarting || this.continuousActive;
  }

  private async cancelAcceptedContinuousJog(): Promise<void> {
    this.continuousActive = false;
    try {
      await this.gateway.cancelJog();
    } catch (error) {
      const message = String(error).toLowerCase();
      if (!message.includes("jog cancel requires") && !message.includes("current mode is idle")) {
        throw error;
      }
    }
  }
}
