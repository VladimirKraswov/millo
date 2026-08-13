import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { Heightmap } from "../../shared/heightmap";
import { defaultHeightmapRequest } from "./heightmapDefaults";
import { buildHeightmapPlan } from "./heightmapModel";
import { HeightmapValues } from "./HeightmapValues";

describe("HeightmapValues", () => {
  it("renders real XY coordinates, absolute Z and deviation without NaN placeholders", () => {
    const plan = buildHeightmapPlan({
      ...defaultHeightmapRequest(),
      originXMm: -2,
      originYMm: 4,
      widthMm: 4,
      heightMm: 2,
      columns: 3,
      rows: 2,
    });
    const map: Heightmap = {
      schemaVersion: 1,
      plan,
      samples: plan.points.map((point) => ({
        point,
        zMm: -0.2 + point.column * 0.01 + point.row * 0.02,
        triggered: true,
      })),
    };

    const markup = renderToStaticMarkup(<HeightmapValues map={map} />);

    expect(markup).toContain("-2.00");
    expect(markup).toContain("0.00");
    expect(markup).toContain("2.00");
    expect(markup).toContain("4.00");
    expect(markup).toContain("6.00");
    expect(markup).toContain("-0.200");
    expect(markup).toContain("+0.010");
    expect(markup.toLowerCase()).not.toContain("nan");
  });

  it("uses a dash for points that have not been measured yet", () => {
    const plan = buildHeightmapPlan({ ...defaultHeightmapRequest(), columns: 2, rows: 2 });
    const map: Heightmap = { schemaVersion: 1, plan, samples: [null, null, null, null] };

    const markup = renderToStaticMarkup(<HeightmapValues map={map} />);

    expect(markup).toContain("—");
    expect(markup.toLowerCase()).not.toContain("nan");
  });
});
