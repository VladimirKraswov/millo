import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { emptySnapshot } from "../../shared/machine";
import { OperatorConsole } from "./OperatorConsole";

describe("OperatorConsole", () => {
  it("renders a compact read-only query palette without an unsafe-mode control", () => {
    const markup = renderToStaticMarkup(
      <OperatorConsole
        desktopRuntime={true}
        onClose={() => undefined}
        onSnapshot={() => undefined}
        open={true}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          machine: { ...emptySnapshot.machine, mode: "idle", reportedMode: "Idle" },
        }}
      />,
    );

    expect(markup).toContain("Операторская консоль");
    expect(markup).toContain("Только чтение");
    for (const command of ["?", "$I", "$$", "$G", "$#"]) {
      expect(markup).toContain(`>${command}</code>`);
    }
    expect(markup).not.toContain("Небезопасный режим");
    expect(markup).not.toContain("raw");
  });
});
