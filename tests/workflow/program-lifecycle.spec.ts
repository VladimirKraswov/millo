import { expect, test } from "@playwright/test";

const harnessPath = "/tests/workflow/programLifecycleHarness.tsx";
async function invoke(page: import("@playwright/test").Page, code: string) {
  return page.evaluate(async ({ path, code }) => {
    const h = await import(/* @vite-ignore */ path);
    return new Function("h", `return (async () => { ${code} })()`)(h);
  }, { path: harnessPath, code });
}
test.beforeEach(async ({ page }) => { await page.goto("/"); });

test("stale block-delete parse cannot replace a newly published job", async ({ page }) => {
  await invoke(page, "h.mountWorkspace();");
  await invoke(page, "void h.workspace.updateExecutionOption('blockDelete', true);");
  await expect(page.getByLabel("workspace")).toContainText('"loading":true');
  await invoke(page, "h.updateWorkspace({incomingJob: {sequence: 1, job: {sourceName: h.program.sourceName, program: h.program, source: 'NEW CONTENT'}}});");
  await invoke(page, "h.parses[0].resolve({...h.program, blockDeleteEnabled: true});");
  await expect(page.getByLabel("workspace")).toContainText('"source":"NEW CONTENT"');
  await expect(page.getByLabel("workspace")).toContainText('"loading":false');
});

test("profile change drops stale preflight success and inspection", async ({ page }) => {
  await invoke(page, "h.mountWorkspace();");
  await invoke(page, "h.workspace.runReadinessAction('runPreflight');");
  await expect(page.getByLabel("workspace")).toContainText('"preflightLoading":true');
  await invoke(page, "h.updateWorkspace({machineContext: {...h.workspaceProps.machineContext, machineProfileId: 'two'}}); h.preflights[0].resolve(h.report);");
  await expect(page.getByLabel("workspace")).not.toContainText('"report":');
  expect(await invoke(page, "return h.inspections.length;")).toBe(0);
  await expect(page.getByLabel("workspace")).toContainText('"preflightLoading":false');
});

test("completed check waits for its command reply, then refreshes readiness once", async ({ page }) => {
  await invoke(page, "h.mountWorkspace();");
  await invoke(page, "h.workspace.startCheckRun();");
  await expect(page.getByLabel("workspace")).toContainText('"checkRunAvailable":false');
  await invoke(page, `h.emitSender({...h.idleSenderSnapshot, sourceName: h.program.sourceName, runSequence: 4, mode: 'checkRun', state: 'completed'});`);
  expect(await invoke(page, "return h.preflights.length;")).toBe(0);
  await invoke(page, `h.commands[0].resolve({...h.idleSenderSnapshot, sourceName: h.program.sourceName, runSequence: 4, mode: 'checkRun', state: 'running'});`);
  await expect.poll(() => invoke(page, "return h.preflights.length;")).toBe(1);
  await invoke(page, "h.preflights[0].resolve(h.report);");
  await expect(page.getByLabel("workspace")).toContainText('"report":');
  await expect(page.getByLabel("workspace")).toContainText('"checkRunVisible":false');
  await expect(page.getByLabel("workspace")).toContainText('"senderState":"completed"');
});

test("old tool-change completion does not close the next confirmation", async ({ page }) => {
  await invoke(page, "h.mountToolChange();");
  await page.getByRole("checkbox").check();
  await page.getByRole("button", { name: "Проверить и продолжить" }).click();
  await invoke(page, "h.nextToolChange(); h.pendingDialog.resolve();");
  await expect(page.getByRole("checkbox")).not.toBeChecked();
  await expect(page.getByRole("button", { name: "Проверить и продолжить" })).toBeDisabled();
  expect(await invoke(page, "return h.dialogClosed;")).toBe(0);
});

test("closed authorization cannot dispatch a run after its response arrives", async ({ page }) => {
  await invoke(page, "h.mountFirstCut();");
  for (const checkbox of await page.getByRole("checkbox").all()) await checkbox.check();
  await page.getByRole("button", { name: "Начать проверку движения" }).click();
  await invoke(page, "h.mountFirstCut(false); h.pendingDialog.resolve({report: h.report, authorization: {id: 1}});");
  expect(await invoke(page, "return h.startedCount;")).toBe(0);
  expect(await invoke(page, "return h.preparedCount;")).toBe(0);
});

test("unmounted recovery does not load its late prepared package", async ({ page }) => {
  await invoke(page, "await h.mountRecovery();");
  await page.getByRole("checkbox").check();
  await page.getByRole("button", { name: "Подготовить повторный запуск" }).click();
  await invoke(page, "h.unmount(); h.pendingDialog.resolve({});");
  expect(await invoke(page, "return h.preparedCount;")).toBe(0);
});

test("editor reparses changed block-delete options without a source edit", async ({ page }) => {
  await invoke(page, "h.mountEditor(); h.mountEditor(true);");
  await expect(page.getByRole("button", { name: "Применить к заданию" })).toBeDisabled();
  await expect.poll(() => invoke(page, "return h.parses.length;")).toBe(1);
  await invoke(page, "h.parses[0].resolve({...h.program, blockDeleteEnabled: true});");
  await expect(page.locator(".program-editor-parse-state")).toHaveAttribute("data-state", "ready");
});

test("probe settings stay locked during save/probe and old profile saves cannot start motion", async ({ page }) => {
  await invoke(page, "h.mountProbe();");
  await page.getByRole("checkbox").check();
  await page.getByRole("button", { name: "Найти поверхность и установить Z", exact: true }).click();
  for (const input of await page.getByRole("spinbutton").all()) await expect(input).toBeDisabled();
  await expect(page.getByRole("button", { name: "Закрыть", exact: true })).toBeDisabled();
  await invoke(page, "h.mountProbe('two'); h.pendingSave.resolve();");
  expect(await invoke(page, "return h.probeRuns;")).toBe(0);
});

test("height-map subscriptions dispose even when registration resolves after unmount", async ({ page }) => {
  await invoke(page, "h.mountHeightmap(); h.unmount(); h.resolveSubscriptions();");
  await expect.poll(() => invoke(page, "return h.unsubscribeCount;")).toBe(await invoke(page, "return h.subscriptions.length;"));
});

test("late height-map initial reads do not replace a running operation event", async ({ page }) => {
  await invoke(page, "h.mountHeightmap(); h.resolveSubscriptions();");
  await invoke(page, "h.staleHeightmapRead();");
  await expect(page.getByText("Снимаю карту", { exact: true })).toBeVisible();
});

test("preview retains its renderer on callback changes and remains nonblank after geometry changes", async ({ page }) => {
  test.setTimeout(90_000);
  await invoke(page, "h.mountPreview();");
  const preview = page.locator(".toolpath-preview canvas");
  await expect(preview).toBeVisible({ timeout: 30_000 });
  const canvas = await preview.elementHandle();
  await invoke(page, "h.mountPreview();");
  expect(await canvas!.evaluate(node => node.isConnected)).toBe(true);
  await invoke(page, "h.mountPreview(1, 0.05, true);");
  await expect(preview).toHaveAttribute("aria-label", /выбрана строка 4, фреза G54 X 10.000/);
  await page.screenshot({ path: test.info().outputPath("preview.png") });
  const screenshot = (await page.locator(".toolpath-preview canvas").screenshot()).toString("base64");
  const pixels = await page.evaluate(async (base64) => {
    const image = new Image(); image.src = `data:image/png;base64,${base64}`; await image.decode();
    const canvas = document.createElement("canvas"); canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext("2d")!; context.drawImage(image, 0, 0);
    const { data } = context.getImageData(0, 0, image.width, image.height);
    const colors = new Set<number>();
    for (let index = 0; index < data.length; index += 4) colors.add((data[index] << 16) | (data[index + 1] << 8) | data[index + 2]);
    return colors.size;
  }, screenshot);
  expect(pixels).toBeGreaterThan(30);
});
