import { describe, expect, it } from "vitest";

import type { PcbInspectRequest } from "../shared/jobs";
import { previewPcbImageJobGateway } from "./previewPcbImageJobGateway";

const request: PcbInspectRequest = {
  files: [
    { sourceName: "board.gtl", sourceBase64: "", role: "copper" },
    { sourceName: "board.drl", sourceBase64: "", role: "drill" },
  ],
  transform: { offsetXMm: 2, offsetYMm: 3, rotationQuarterTurns: 0, mirrorX: false },
};

describe("previewPcbImageJobGateway", () => {
  it("renders drill groups and slots without a Tauri runtime", async () => {
    const inspection = await previewPcbImageJobGateway.inspectPcb(request);

    expect(inspection.bounds.minXMm).toBe(2);
    expect(inspection.drillGroups).toHaveLength(2);
    expect(inspection.drillHits).toHaveLength(2);
    expect(inspection.drillSlots).toHaveLength(1);
  });
});
