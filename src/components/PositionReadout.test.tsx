import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { formatCoordinate, PositionReadout } from "./PositionReadout";

describe("PositionReadout", () => {
  it("renders XYZ and adds A only when the controller reports it", () => {
    const xyz = renderToStaticMarkup(
      <PositionReadout position={{ x: 1, y: 2, z: -0.125 }} />,
    );
    const xyza = renderToStaticMarkup(
      <PositionReadout position={{ x: 1, y: 2, z: 3, a: 4 }} />,
    );

    expect(xyz).toContain("-0.125");
    expect(xyz).not.toContain(">A<");
    expect(xyza).toContain(">A<");
    expect(xyza).toMatch(/>A<\/span><strong>4\.000<\/strong><small>°<\/small>/);
    expect(formatCoordinate(undefined)).toBe("--");
  });
});
