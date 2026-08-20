import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { emptySnapshot } from "../../shared/machine";
import { OperatorConsole } from "./OperatorConsole";

describe("OperatorConsole", () => {
  it("renders the safe query palette when safe command mode is enabled", () => {
    const markup = renderToStaticMarkup(
      <OperatorConsole
        desktopRuntime={true}
        onClose={() => undefined}
        onSnapshot={() => undefined}
        open={true}
        safeCommandMode={true}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          machine: { ...emptySnapshot.machine, mode: "idle", reportedMode: "Idle" },
        }}
      />,
    );

    expect(markup).toContain("Операторская консоль");
    expect(markup).toContain("Безопасный режим");
    for (const command of ["?", "$I", "$$", "$G", "$#"]) {
      expect(markup).toContain(`>${command}</code>`);
    }
    expect(markup).not.toContain("Экспертный режим");
    expect(markup).not.toContain("raw");
  });

  it("keeps the safe palette visible and clearly marks expert mode", () => {
    const markup = renderToStaticMarkup(
      <OperatorConsole
        desktopRuntime={true}
        onClose={() => undefined}
        onSnapshot={() => undefined}
        open={true}
        safeCommandMode={false}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          machine: { ...emptySnapshot.machine, mode: "idle", reportedMode: "Idle" },
        }}
      />,
    );

    expect(markup).toContain("Экспертный режим");
    expect(markup).toContain('placeholder="G0 X10"');
    expect(markup).toContain(">$I</code>");
  });
});
