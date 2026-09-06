import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  previewFixtureCheckCompleteSender,
  previewFixtureCompletedSender,
  previewFixtureToolChangeSender,
} from "./previewFixtureFirstCut";
import { physicalSenderActionLayout } from "./operatorLayoutModel";
import { ProgramRunCard } from "./ProgramRunCard";

const actions = {
  onCancelCheck: () => undefined,
  onPause: () => undefined,
  onPrepareRerun: () => undefined,
  onResolveInterruption: () => undefined,
  onResume: () => undefined,
  onReturnFromCheck: () => undefined,
  onReturnToWorkOrigin: () => undefined,
  onStop: () => undefined,
  onToolChange: () => undefined,
};

describe("ProgramRunCard", () => {
  it("locks interruption resolution while another sender action is pending", () => {
    const markup = renderToStaticMarkup(
      <ProgramRunCard {...actions} busy checkAction="none" checkControlsAvailable
        checkRun={false} machineContextAvailable physicalActions={physicalSenderActionLayout("failed")}
        programControlsAvailable programRun progressPercent={50} recoveryAvailable recoveryChecked
        sender={{ ...previewFixtureCompletedSender, state: "failed" }} />,
    );
    expect(markup).toMatch(/class="is-terminal-action" disabled=""/);
  });
  it("offers rerun and safe work-zero return after completed processing", () => {
    const markup = renderToStaticMarkup(
      <ProgramRunCard
        {...actions}
        busy={false}
        checkAction="none"
        checkControlsAvailable
        checkRun={false}
        machineContextAvailable
        physicalActions={physicalSenderActionLayout(previewFixtureCompletedSender.state)}
        programControlsAvailable
        programRun
        progressPercent={100}
        recoveryAvailable={false}
        recoveryChecked
        sender={previewFixtureCompletedSender}
      />,
    );

    expect(markup).toContain("Обработка");
    expect(markup).toContain("Вернуться в рабочий ноль");
    expect(markup).toContain("Подготовить повторный запуск");
  });

  it("keeps tool change as a host-owned action", () => {
    const markup = renderToStaticMarkup(
      <ProgramRunCard
        {...actions}
        busy={false}
        checkAction="none"
        checkControlsAvailable
        checkRun={false}
        machineContextAvailable
        physicalActions={physicalSenderActionLayout(previewFixtureToolChangeSender.state)}
        programControlsAvailable
        programRun
        progressPercent={50}
        recoveryAvailable={false}
        recoveryChecked
        sender={previewFixtureToolChangeSender}
      />,
    );

    expect(markup).toContain("Подтвердить замену");
    expect(markup).toContain("M6 удерживается приложением");
  });

  it("labels completed GRBL Check without physical-run recovery actions", () => {
    const markup = renderToStaticMarkup(
      <ProgramRunCard
        {...actions}
        busy={false}
        checkAction="none"
        checkControlsAvailable
        checkRun
        machineContextAvailable
        physicalActions={physicalSenderActionLayout(previewFixtureCheckCompleteSender.state)}
        programControlsAvailable
        programRun={false}
        progressPercent={100}
        recoveryAvailable={false}
        recoveryChecked
        sender={previewFixtureCheckCompleteSender}
      />,
    );

    expect(markup).toContain("Проверка GRBL");
    expect(markup).toContain("Все строки приняты в $C");
    expect(markup).not.toContain("Вернуться в рабочий ноль");
  });

  it("hides host-owned controls when their gateway is unavailable", () => {
    const toolChangeMarkup = renderToStaticMarkup(
      <ProgramRunCard
        {...actions}
        busy={false}
        checkAction="none"
        checkControlsAvailable={false}
        checkRun={false}
        machineContextAvailable
        physicalActions={physicalSenderActionLayout(previewFixtureToolChangeSender.state)}
        programControlsAvailable={false}
        programRun
        progressPercent={50}
        recoveryAvailable={false}
        recoveryChecked
        sender={previewFixtureToolChangeSender}
      />,
    );

    expect(toolChangeMarkup).not.toContain("Подтвердить замену");
  });
});
