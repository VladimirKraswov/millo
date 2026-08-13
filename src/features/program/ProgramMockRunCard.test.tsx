import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { idleSenderSnapshot } from "../../shared/dryRun";
import { ProgramMockRunCard } from "./ProgramMockRunCard";
import { dryRunControls } from "./dryRunReadModel";
import { senderActionLayout } from "./operatorLayoutModel";

const actions = {
  onCancel: () => undefined,
  onPrimary: () => undefined,
};

describe("ProgramMockRunCard", () => {
  it("shows one eligible Mock start action", () => {
    const controls = dryRunControls(idleSenderSnapshot, {
      loading: false,
      mockAvailable: true,
      policyEligible: true,
    });
    const markup = renderToStaticMarkup(
      <ProgramMockRunCard
        {...actions}
        actions={senderActionLayout(idleSenderSnapshot.state)}
        controls={controls}
        dryRunAvailable
        failure={false}
        gatewayAvailable
        sender={idleSenderSnapshot}
        status="Готово"
      />,
    );

    expect(markup).toContain("Запустить тест");
    expect(markup).toContain('aria-valuenow="0"');
    expect(markup).toContain("Готово");
  });

  it("keeps active cancellation fail-closed without a gateway", () => {
    const sender = { ...idleSenderSnapshot, state: "running" as const, progress: 0.5 };
    const markup = renderToStaticMarkup(
      <ProgramMockRunCard
        {...actions}
        actions={senderActionLayout(sender.state)}
        controls={dryRunControls(sender, {
          loading: false,
          mockAvailable: true,
          policyEligible: true,
        })}
        dryRunAvailable
        failure={false}
        gatewayAvailable={false}
        sender={sender}
        status="Выполнение"
      />,
    );

    expect(markup).toContain("Пауза");
    expect(markup).toContain("Отменить");
    expect(markup.match(/disabled=""/g)).toHaveLength(2);
  });
});
