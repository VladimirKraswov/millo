import { expect, test, type Locator, type Page } from "@playwright/test";

// Keep the resource-test budget when this spec runs in the full workflow suite.
test.setTimeout(90_000);

const lastA = "N1000000 G1 X18 Y12 Z-1 A810 F240 ; END-A";
const lastB = "N1000000 G1 X2 Y14 Z-2 A-270 F120 ; END-B";

async function invoke(page: Page, code: string) {
  return page.evaluate(async (code) => {
    const path = "/tests/workflow/program-documents-harness.tsx";
    const harness = await import(/* @vite-ignore */ path);
    return new Function("h", code)(harness);
  }, code);
}

async function jump(page: Page, line: number) {
  const input = page.getByRole("spinbutton", { name: "Перейти к строке" });
  await input.fill(String(line));
  await input.press("Enter");
}

async function expectLastLine(page: Page, source: string, degrees: number) {
  await expect(page.locator(".program-page-toolbar > span")).toHaveText("999937–1000000 / 1000000");
  const row = page.getByRole("option", { name: `Строка 1000000: ${source}`, exact: true });
  await expect(row).toHaveAttribute("aria-selected", "true");
  await expect(row.locator("code")).toHaveAttribute("title", source);
  await expect(page.getByRole("status")).toContainText("L1000000");
  await expect(page.getByRole("status").locator("code")).toHaveAttribute("title", source);
  await expect(page.getByLabel("Поворот A выбранной строки")).toContainText(`Конец ${degrees.toFixed(3)}°`);
  await expect(page.getByRole("button", { name: "Следующие строки" })).toBeDisabled();
}

async function expectBoundedLayout(page: Page) {
  await page.getByRole("spinbutton", { name: "Перейти к строке" }).scrollIntoViewIfNeeded();
  expect(await page.getByRole("option").count()).toBeLessThan(40);
  expect(await page.locator(".program-line-spacer").evaluate((element) => element.getBoundingClientRect().height)).toBeLessThanOrEqual(512 * 34);
  const layout = await page.evaluate(() => {
    const bounds = [".program-page-toolbar", ".program-line-table", ".program-preview-stage", ".preview-selection",
      ".program-rotary-metrics", ".program-metrics"].map((selector) => document.querySelector(selector)!.getBoundingClientRect());
    const input = document.querySelector<HTMLInputElement>(".program-page-toolbar input")!;
    const inputBounds = input.getBoundingClientRect();
    const toolbarChildren = [...document.querySelectorAll(".program-page-toolbar > *")]
      .map((element) => element.getBoundingClientRect());
    return { pageFits: document.documentElement.scrollWidth <= innerWidth,
      fits: bounds.every((rect) => rect.left >= 0 && rect.right <= innerWidth),
      separateMetrics: bounds[3].bottom <= bounds[4].top && bounds[4].bottom <= bounds[5].top,
      toolbarChildrenFit: toolbarChildren.every((rect) => rect.top >= bounds[0].top && rect.bottom <= bounds[0].bottom),
      tableFollowsToolbar: bounds[0].bottom <= bounds[1].top,
      inputReceivesPointer: document.elementFromPoint(inputBounds.x + inputBounds.width / 2,
        inputBounds.y + inputBounds.height / 2) === input };
  });
  expect(layout).toEqual({ pageFits: true, fits: true, separateMetrics: true,
    toolbarChildrenFit: true, tableFollowsToolbar: true, inputReceivesPointer: true });
}

async function expectGeometry(page: Page, canvas: Locator) {
  const colors = await page.evaluate(async (base64) => {
    const image = new Image();
    image.src = `data:image/png;base64,${base64}`;
    await image.decode();
    const decoded = document.createElement("canvas");
    decoded.width = image.width; decoded.height = image.height;
    const context = decoded.getContext("2d")!;
    context.drawImage(image, 0, 0);
    const { data } = context.getImageData(0, 0, image.width, image.height);
    const colors = new Set<number>();
    for (let i = 0; i < data.length; i += 4) colors.add((data[i] << 16) | (data[i + 1] << 8) | data[i + 2]);
    return colors.size;
  }, (await canvas.screenshot()).toString("base64"));
  expect(colors).toBeGreaterThan(30);
}

test("million-row pages preserve exact selection, source recovery request and zoomed 3D scene", async ({ page }, info) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await invoke(page, "h.mount();");
  const canvas = page.locator(".toolpath-canvas canvas");
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  const retainedCanvas = await canvas.elementHandle();
  await expect(page.getByText("Обзорная траектория", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Предыдущие строки" })).toBeDisabled();
  await page.getByRole("button", { name: "Следующие строки" }).click();
  await expect(page.locator(".program-page-toolbar > span")).toHaveText("513–1024 / 1000000");
  await expect(page.getByRole("option", { name: "Строка 513: G1 X0 Y0 Z0 F240", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Предыдущие строки" }).click();
  await expect(page.locator(".program-page-toolbar > span")).toHaveText("1–512 / 1000000");

  await invoke(page, 'h.holdRequests("detail");');
  await jump(page, 1_000_000);
  await expect(page.getByRole("option", { name: `Строка 1000000: ${lastA}`, exact: true })).toBeVisible();
  await expect.poll(() => invoke(page, "return h.pendingCounts();")).toEqual({ page: 0, detail: 1 });
  const overview = await canvas.screenshot();
  await invoke(page, 'return h.flushHeld("success");');
  await expectLastLine(page, lastA, 810);
  expect(await retainedCanvas!.evaluate((element) => element.isConnected)).toBe(true);
  expect((await canvas.screenshot()).equals(overview)).toBe(false);
  expect(await invoke(page, "return h.snapshot;")).toMatchObject({ programId: "document-a", sourceLine: 1_000_000,
    line: { source: lastA }, toolpath: [{ sourceLine: 1_000_000, points: [{ x: 20, y: 15, z: 0 }, { x: 18, y: 12, z: -1 }],
      rotary: { startDegrees: 0, endDegrees: 810 } }] });
  await expectGeometry(page, canvas);

  const initialView = await canvas.screenshot();
  await canvas.hover();
  await page.mouse.wheel(0, -180);
  await expect.poll(async () => (await canvas.screenshot()).equals(initialView)).toBe(false);
  const zoomed = await canvas.screenshot();
  await page.getByRole("button", { name: "Предыдущие строки" }).click();
  await expect(page.locator(".program-page-toolbar > span")).toHaveText("999425–999936 / 1000000");
  await expect(page.getByRole("listbox")).toBeVisible();
  await page.getByRole("button", { name: "Следующие строки" }).click();
  await expectLastLine(page, lastA, 810);
  expect(await retainedCanvas!.evaluate((element) => element.isConnected)).toBe(true);
  expect((await canvas.screenshot()).equals(zoomed)).toBe(true);
  const requests = await invoke(page, "return h.requests;");
  expect(requests.length).toBeGreaterThanOrEqual(5);
  expect(requests.every((request: { sourceMatches: boolean; sourceLength: number; blockDelete: boolean }) =>
    request.sourceMatches && request.sourceLength > 15_000_000 && request.blockDelete)).toBe(true);
  expect(requests.filter((request: { kind: string }) => request.kind === "page")
    .every((request: { count: number }) => request.count === 512)).toBe(true);
  await expectBoundedLayout(page);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await page.screenshot({ path: info.outputPath("program-documents.png"), fullPage: true });
  expect(errors).toEqual([]);
});

test("replacement ignores late page/detail success and failure without resetting the new scene", async ({ page }, info) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await invoke(page, "h.mount();");
  const canvas = page.locator(".toolpath-canvas canvas");
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  const oldCanvas = await canvas.elementHandle();
  await invoke(page, 'h.holdRequests("all");');
  // The late successful reply has the same line/page number as the new document.
  await jump(page, 1_000_000);
  await expect.poll(() => invoke(page, "return h.pendingCounts();")).toEqual({ page: 1, detail: 1 });
  await jump(page, 900_001);
  await expect.poll(() => invoke(page, "return h.pendingCounts();")).toEqual({ page: 2, detail: 2 });
  await invoke(page, "h.replaceDocument();");
  await expect(page.getByRole("heading", { name: "b-million.nc" })).toBeVisible();
  // The legitimate replacement scene initializes before the passive page reset.
  await expect(page.locator(".program-page-toolbar > span")).toHaveText("1–512 / 1000000", { timeout: 30_000 });
  await expect.poll(() => oldCanvas!.evaluate((element) => element.isConnected)).toBe(false);
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  const retainedCanvas = await canvas.elementHandle();
  await jump(page, 1_000_000);
  await expectLastLine(page, lastB, -270);
  const before = await canvas.screenshot();
  await invoke(page, 'return h.flushHeld("mixed");');
  await expectLastLine(page, lastB, -270);
  expect(await invoke(page, "return h.pendingCounts();")).toEqual({ page: 0, detail: 0 });
  expect(await invoke(page, "return h.snapshot;")).toMatchObject({ programId: "document-b", sourceLine: 1_000_000,
    line: { source: lastB }, toolpath: [{ sourceLine: 1_000_000, points: [{ x: 22, y: 17, z: 0 }, { x: 2, y: 14, z: -2 }],
      rotary: { startDegrees: 0, endDegrees: -270 } }] });
  expect(await retainedCanvas!.evaluate((element) => element.isConnected)).toBe(true);
  expect((await canvas.screenshot()).equals(before)).toBe(true);
  const requests = await invoke(page, "return h.requests;");
  expect(new Set(requests.map((request: { programId: string }) => request.programId)).size).toBe(2);
  expect(requests.every((request: { programId: string; sourceMatches: boolean; blockDelete: boolean }) =>
    request.sourceMatches && request.blockDelete === (request.programId === "document-a"))).toBe(true);
  await expectBoundedLayout(page);
  await expectGeometry(page, canvas);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await page.screenshot({ path: info.outputPath("program-documents-replaced.png"), fullPage: true });
  expect(errors).toEqual([]);
});
