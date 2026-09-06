import { expect, test, type Page } from "@playwright/test";

async function invoke(page: Page, code: string) {
  return page.evaluate(async (code) => {
    const path = "/tests/workflow/rotaryPreviewHarness.tsx";
    const h = await import(/* @vite-ignore */ path);
    return new Function("h", code)(h);
  }, code);
}

test("rotary projection renders, picks A-only moves and updates telemetry without a program scan", async ({ page }, info) => {
  test.setTimeout(90_000);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await invoke(page, "h.mount();");
  const canvas = page.locator(".toolpath-canvas canvas");
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText("Проекция XYZ", { exact: true })).toBeVisible();
  await expect(page.getByText("Обзорная траектория", { exact: true })).toBeVisible();
  const a = page.getByLabel("Текущее положение оси A");
  await expect(a).toHaveText("A -810.250°");
  const reads = await invoke(page, "return h.toolpathReads;");
  const geometryScreenshot = () => canvas.screenshot({ mask: [page.locator(".tool-position-hud")] });
  const before = await geometryScreenshot();
  await invoke(page, "h.updateAngle(1080.5);");
  await expect(a).toHaveText("A 1080.500°");
  expect(await invoke(page, "return h.toolpathReads;")).toBe(reads);
  expect(Buffer.compare(await geometryScreenshot(), before)).toBe(0);

  const point = await invoke(page, "return h.markerPosition();");
  await page.mouse.click(point.x, point.y);
  await expect(page.getByRole("status")).toContainText("L99");
  await expect(page.getByLabel("Поворот A выбранной строки")).toContainText("Начало -90.000°");
  await expect(page.getByLabel("Поворот A выбранной строки")).toContainText("Конец 720.000°");
  await invoke(page, "h.updateAngle(undefined);");
  await expect(a).toHaveText("A --");

  const retainedCanvas = await canvas.elementHandle();
  const selectedBefore = await canvas.screenshot();
  const readsBeforeDetail = await invoke(page, "return h.toolpathReads;");
  await invoke(page, "h.selectDetail();");
  await expect(page.getByRole("status")).toContainText("L100");
  await expect(page.getByLabel("Поворот A выбранной строки")).toContainText("Конец 1080.000°");
  expect(await retainedCanvas!.evaluate((element) => element.isConnected)).toBe(true);
  expect(await invoke(page, "return h.toolpathReads;")).toBe(readsBeforeDetail);
  expect(Buffer.compare(selectedBefore, await canvas.screenshot())).not.toBe(0);

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
  const top = await canvas.screenshot();
  const previousCanvas = await canvas.elementHandle();
  await invoke(page, "h.changeView();");
  await expect.poll(() => previousCanvas!.evaluate((element) => element.isConnected)).toBe(false);
  await expect(canvas).toBeVisible();
  await expect.poll(async () => Buffer.compare(top, await canvas.screenshot())).not.toBe(0);

  const layout = await page.evaluate(() => {
    const selectors = [".preview-selection", ".tool-position-hud", ".program-rotary-metrics", ".program-metrics"];
    const bounds = selectors.map((selector) => document.querySelector(selector)!.getBoundingClientRect());
    return { fits: bounds.every((rect) => rect.left >= 0 && rect.right <= innerWidth),
      separated: bounds[0].bottom <= bounds[1].top && bounds[1].bottom <= bounds[2].top && bounds[2].bottom <= bounds[3].top,
      pageFits: document.documentElement.scrollWidth <= innerWidth };
  });
  expect(layout).toEqual({ fits: true, separated: true, pageFits: true });
  await page.screenshot({ path: info.outputPath("rotary-preview.png"), fullPage: true });
  expect(errors).toEqual([]);
});
