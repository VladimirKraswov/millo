import { renderToStaticMarkup } from "react-dom/server";
import type { ComponentProps } from "react";
import { describe, expect, it } from "vitest";

import { emptySnapshot, type TransportDescriptor } from "../../shared/machine";
import { ConnectionPanel } from "./ConnectionPanel";

type Props = ComponentProps<typeof ConnectionPanel>;

const serialTransport: TransportDescriptor = {
  id: "serial:/dev/cu.usbmodem101",
  kind: "serial",
  label: "/dev/cu.usbmodem101",
  detail: "USB CNC controller",
  likelyGrbl: true,
};

const actions: Props["actions"] = {
  onBaudRate: () => undefined,
  onConnect: () => undefined,
  onDisconnect: () => undefined,
  onDismissError: () => undefined,
  onLikelyGrblOnly: () => undefined,
  onOpenLog: () => undefined,
  onRefreshStatus: () => undefined,
  onRefreshTransports: () => undefined,
  onTransport: () => undefined,
};

const baseView: Props["view"] = {
  baudRate: 115_200,
  canDisconnect: false,
  controlsBusy: false,
  desktopRuntime: true,
  discovering: false,
  displayedTransport: serialTransport,
  hasConnection: false,
  isConnected: false,
  likelyGrblOnly: true,
  selectedTransport: serialTransport,
  snapshot: emptySnapshot,
  transportLocked: false,
  visibleTransports: [serialTransport],
};

describe("ConnectionPanel", () => {
  it("keeps transport selection visible before connection", () => {
    const markup = renderToStaticMarkup(
      <ConnectionPanel actions={actions} view={baseView} />,
    );

    expect(markup).toContain("Подключить");
    expect(markup).toContain("Только вероятные GRBL");
    expect(markup).not.toContain("Запросить статус");
  });

  it("shows operator controls and diagnostics for an active connection", () => {
    const markup = renderToStaticMarkup(
      <ConnectionPanel
        actions={actions}
        controls={<div>Jog fixture</div>}
        view={{
          ...baseView,
          canDisconnect: true,
          hasConnection: true,
          isConnected: true,
          selectedMachineName: "LUNYEE CNC",
          snapshot: { ...emptySnapshot, connection: "connected" },
          transportLocked: true,
        }}
      />,
    );

    expect(markup).toContain("Станок: LUNYEE CNC");
    expect(markup).toContain("Jog fixture");
    expect(markup).toContain("Запросить статус");
    expect(markup).not.toContain("Сценарии Mock GRBL");
    expect(markup).not.toContain(">Подключить<");
  });

  it("keeps a transport failure discoverable until the operator dismisses it", () => {
    const markup = renderToStaticMarkup(
      <ConnectionPanel
        actions={actions}
        view={{ ...baseView, displayedError: "Serial link lost" }}
      />,
    );

    expect(markup).toContain('role="alert"');
    expect(markup).toContain("Serial link lost");
    expect(markup).toContain("Журнал");
    expect(markup).toContain("Закрыть ошибку");
  });
});
