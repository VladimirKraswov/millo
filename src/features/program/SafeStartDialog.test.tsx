import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { SafeStartDialog } from "./SafeStartDialog";

describe("SafeStartDialog", () => {
  it("explains safe rewind and the mandatory GRBL Check", () => {
    const markup = renderToStaticMarkup(
      <SafeStartDialog
        minimumSafeZ={5}
        motionCount={1}
        onClose={() => undefined}
        onPrepare={async () => {
          throw new Error("not used during server render");
        }}
        onPrepared={() => undefined}
        open
        selectedCommand="G1 X30 F100"
        sourceLine={42}
        suggestedSafeZ={7}
      />,
    );

    expect(markup).toContain("Запустить с выбранного участка");
    expect(markup).toContain("L42");
    expect(markup).toContain("последний безопасный rapid-вход");
    expect(markup).toContain("Следующий шаг: GRBL Check");
    expect(markup).toContain("Подготовить и запустить Check");
  });
});
