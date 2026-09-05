import { describe, expect, it } from "vitest";
import { emptySnapshot, type ControllerSnapshot } from "../../shared/machine";
import { isProbeDatumCurrent, type ProbeEstablishedZDatum } from "./probeDatumModel";

const datum: ProbeEstablishedZDatum = {
  profileId: "machine", coordinateSystem: "g54", resetCount: 0, reconnectCount: 0,
  source: "probe", workCoordinateOffsetZ: 19.4,
};
const snapshot: ControllerSnapshot = {
  ...emptySnapshot, connection: "connected",
  machine: { ...emptySnapshot.machine, workCoordinateOffset: { x: 0, y: 0, z: 19.4 } },
};

describe("established probe datum", () => {
  it("survives equal telemetry, XY zeroing and sparse status", () => {
    for (const workCoordinateOffset of [{ x: 12, y: 6, z: 19.4 }, undefined]) {
      expect(isProbeDatumCurrent(datum, { ...snapshot, pollSequence: 20,
        machine: { ...snapshot.machine, workCoordinateOffset } }, "G54", "machine")).toBe(true);
    }
  });
  it.each([
    { connection: "disconnected" as const },
    { resetCount: 1 },
    { reconnectCount: 1 },
    { machine: { ...snapshot.machine, workCoordinateOffset: { x: 0, y: 0, z: 19.5 } } },
    { machine: { ...snapshot.machine, workCoordinateOffset: { x: 0, y: 0, z: NaN } } },
  ])("invalidates lost or changed evidence: %j", (change) => {
    expect(isProbeDatumCurrent(datum, { ...snapshot, ...change }, "G54", "machine")).toBe(false);
  });
  it("does not transfer a datum to another WCS or machine", () => {
    expect(isProbeDatumCurrent(datum, snapshot, "G55", "machine")).toBe(false);
    expect(isProbeDatumCurrent(datum, snapshot, "G54", "other")).toBe(false);
    expect(isProbeDatumCurrent(undefined, snapshot, "G54", "machine")).toBe(false);
  });
});
