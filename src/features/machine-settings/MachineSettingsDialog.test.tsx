import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { MachineSettingsDialog } from "./MachineSettingsDialog";

const renderPreferences = (safeCommandMode: boolean) =>
  renderToStaticMarkup(
    <MachineSettingsDialog
      applicationPreferences={{ safeCommandMode }}
      initialView="application"
      onApplicationPreferencesUpdate={async (update) => update}
      onClose={() => undefined}
      onLocalUpdate={async () => ({ profiles: [] })}
      onOpenToolLibrary={() => undefined}
      onRollback={async () => {
        throw new Error("unused");
      }}
      onWrite={async () => {
        throw new Error("unused");
      }}
      open={true}
    />,
  );

describe("MachineSettingsDialog application preferences", () => {
  it("enables safe command mode by default", () => {
    const markup = renderPreferences(true);

    expect(markup).toContain("Безопасный режим команд");
    expect(markup).toMatch(
      /aria-label="Безопасный режим команд"[^>]*checked=""/,
    );
    expect(markup).not.toContain("Экспертный режим включён");
  });

  it("keeps expert mode visible while protection is disabled", () => {
    const markup = renderPreferences(false);

    expect(markup).toContain("Экспертный режим включён");
    expect(markup).toContain("machine.commands");
  });
});
