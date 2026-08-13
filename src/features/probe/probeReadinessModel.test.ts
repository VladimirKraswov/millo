import { describe, expect, it } from "vitest";

import { describeProbeReadinessFailure } from "./probeReadinessModel";

describe("probeReadinessModel", () => {
  it("explains a trailing motion without claiming the controller is disconnected", () => {
    expect(
      describeProbeReadinessFailure(
        "probe start timed out after 3000 ms waiting for Idle (last mode Run)",
        "измерению",
      ),
    ).toContain("предыдущее движение");
  });

  it("keeps alarm and disconnect guidance distinct", () => {
    expect(
      describeProbeReadinessFailure(
        "probe start is blocked: connection Connected, mode Alarm, alarm true, reset acknowledgement pending false",
        "касанию",
      ),
    ).toContain("Alarm");
    expect(
      describeProbeReadinessFailure(
        "probe start is blocked: connection Disconnected, mode Unknown, alarm false, reset acknowledgement pending false",
        "касанию",
      ),
    ).toContain("Переподключите");
  });

  it("explains an active sender instead of reporting a connection failure", () => {
    expect(
      describeProbeReadinessFailure("another machine operation is active", "измерению"),
    ).toContain("завершите или отмените текущее задание");
  });
});
