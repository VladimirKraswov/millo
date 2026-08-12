import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ConnectionState } from "../../shared/machine";
import { ProbeIndicator } from "./ProbeIndicator";

const renderIndicator = (connection: ConnectionState, active: boolean) =>
  renderToStaticMarkup(
    <ProbeIndicator active={active} connection={connection} onClick={() => undefined} />,
  );

describe("ProbeIndicator", () => {
  it("shows the live probe input without changing its visible label", () => {
    const open = renderIndicator("connected", false);
    const triggered = renderIndicator("connected", true);

    expect(open).toContain("is-open");
    expect(open).toContain("Щуп: контакт разомкнут");
    expect(triggered).toContain("is-triggered");
    expect(triggered).toContain("Щуп: контакт замкнут");
    expect(open).toContain("<span>Щуп</span>");
    expect(triggered).toContain("<span>Щуп</span>");
    expect(open).toContain("<button");
    expect(open).toContain('type="button"');
  });

  it.each<ConnectionState>(["disconnected", "connecting", "recovering", "faulted"])(
    "does not expose stale probe state while the connection is %s",
    (connection) => {
      const markup = renderIndicator(connection, true);

      expect(markup).toContain("is-unavailable");
      expect(markup).toContain("Щуп: нет актуального статуса");
      expect(markup).not.toContain("is-triggered");
    },
  );
});
