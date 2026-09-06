import { expect, test, type Page } from "@playwright/test";

// Million-line resource regressions also run under the root workflow config.
test.setTimeout(90_000);

async function invoke(page: Page, code: string) {
  return page.evaluate(async (code) => {
    const path = "/tests/workflow/editorLargeHarness.tsx";
    const h = await import(/* @vite-ignore */ path);
    return new Function("h", code)(h);
  }, code);
}

async function jump(page: Page, line: number) {
  await page.getByRole("spinbutton", { name: "Перейти к строке" }).fill(String(line));
  await page.getByRole("spinbutton", { name: "Перейти к строке" }).press("Enter");
  await expect(page.locator(".program-editor-toolbar > code")).toContainText(`L${line} ·`);
}

test("edits million-line source with bounded pages, global undo, clipboard and complete saves", async ({ page }, info) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await invoke(page, "h.mount();");
  const editor = page.getByRole("textbox", { name: "Исходный G-code" });
  await expect(editor).toBeVisible();
  expect(await editor.evaluate((element: HTMLTextAreaElement) => element.value.split("\n").length)).toBe(5000);
  expect(await page.locator(".program-editor-highlight > span").count()).toBeLessThan(100);

  await jump(page, 999999);
  await expect.poll(() => invoke(page, "return h.detailLine;")).toBe(999999);
  expect(await invoke(page, "return {length: h.detailRequest.source.length, options: h.detailRequest.parseOptions};"))
    .toEqual({ length: 6_000_003, options: { blockDelete: false } });
  await editor.press("End");
  expect(await editor.evaluate((element: HTMLTextAreaElement) => element.selectionStart)).toBe(29_993);
  await editor.press("Backspace");
  await page.keyboard.insertText("9");
  await expect(page.getByRole("button", { name: "Сохранить как", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: "Сохранить как", exact: true }).click();
  expect(await invoke(page, "return {length: h.savedSource?.length, tail: h.savedSource?.slice(-40), matches: h.savedSource === h.originalSource.slice(0, 999998 * 6) + 'G1 X9\\nG1 X1\\nM30'};")).toEqual({
    length: 6_000_003, tail: ("G1 X1\n".repeat(6) + "G1 X9\nG1 X1\nM30").slice(-40), matches: true,
  });
  await page.getByRole("button", { name: "Отменить", exact: true }).click();
  await page.getByRole("button", { name: "Отменить", exact: true }).click();
  await expect(page.getByRole("button", { name: "Сохранить как", exact: true })).toBeEnabled();
  await expect.poll(() => invoke(page, "return h.currentSource === h.originalSource;")).toBe(true);
  await expect(page.locator(".program-editor-toolbar > code")).toContainText("L999999 ·");
  await page.getByRole("button", { name: "Повторить", exact: true }).click();
  await page.getByRole("button", { name: "Повторить", exact: true }).click();

  await jump(page, 5001);
  await page.getByRole("button", { name: "Вставить строку", exact: true }).click();
  await expect(page.locator(".program-editor-toolbar > code")).toContainText("1000002 строк");
  expect((await editor.inputValue()).startsWith("\nG1 X1")).toBe(true);
  await page.getByRole("button", { name: "Удалить выбранные строки", exact: true }).click();
  await expect(page.locator(".program-editor-toolbar > code")).toContainText("1000001 строк");
  await editor.press("Backspace");
  await expect(page.locator(".program-editor-toolbar > code")).toContainText("L5000 ·");
  await page.getByRole("button", { name: "Отменить", exact: true }).click();
  await expect(page.locator(".program-editor-toolbar > code")).toContainText("L5001 ·");

  await jump(page, 5000);
  await editor.press("End");
  await editor.press("Shift+ArrowDown");
  await expect(page.locator(".program-editor-toolbar > code")).toContainText("L5001 ·");
  const copied = await editor.evaluate((element) => {
    const clipboardData = new DataTransfer();
    element.dispatchEvent(new ClipboardEvent("copy", { clipboardData, bubbles: true, cancelable: true }));
    return clipboardData.getData("text/plain");
  });
  expect(copied).toBe("\nG1 X1");
  await editor.press("Backspace");
  await expect(page.locator(".program-editor-toolbar > code")).toContainText("1000000 строк");
  await page.getByRole("button", { name: "Отменить", exact: true }).click();
  await expect(page.locator(".program-editor-toolbar > code")).toContainText("1000001 строк");

  await editor.press("ControlOrMeta+a");
  const copyLength = await editor.evaluate((element) => {
    const clipboardData = new DataTransfer();
    element.dispatchEvent(new ClipboardEvent("copy", { clipboardData, bubbles: true, cancelable: true }));
    return clipboardData.getData("text/plain").length;
  });
  expect(copyLength).toBe(6_000_003);
  expect(await editor.evaluate((element: HTMLTextAreaElement) => element.value.split("\n").length)).toBeLessThanOrEqual(5000);
  await jump(page, 1000001);
  await expect(editor).toHaveValue("M30");
  await page.getByRole("button", { name: "Обработанная копия", exact: true }).click();
  await expect.poll(() => invoke(page, "return h.processedSourceName;")).toBe("million-lines-transformed.nc");
  expect(await invoke(page, "return h.processedRequest.source.length;")).toBe(6_000_003);
  expect(await invoke(page, "return h.processedRequest.programId;")).toMatch(/^revision-/);
  expect(await invoke(page, "return h.processedRequest.parseOptions;")).toEqual({ blockDelete: false });
  await page.getByRole("button", { name: "Применить к заданию", exact: true }).click();
  expect(await invoke(page, "return h.applied.source.length;")).toBe(6_000_003);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: info.outputPath("large-editor.png"), fullPage: true });
  expect(errors).toEqual([]);
});

test("processed export reports missing capability instead of saving the visible rows", async ({ page }) => {
  await page.goto("/");
  await invoke(page, "h.mount(false);");
  await page.getByRole("button", { name: "Обработанная копия", exact: true }).click();
  await expect(page.locator(".program-editor-footer-status")).toContainText("полной обработанной копии недоступно");
  expect(await invoke(page, "return h.savedSource;")).toBeUndefined();
});
