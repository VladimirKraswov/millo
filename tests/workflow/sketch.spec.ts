import { expect, test, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const fixture = resolve("target/debug/examples/fixture");
async function mount(page: Page) {
  await page.route("**/__test__/sketch-tools", (route) =>
    route.fulfill({
      contentType: "application/json",
      body: execFileSync(fixture, ["tools"], { encoding: "utf8" }),
    }),
  );
  await page.route("**/__test__/sketch-cam", async (route) => {
    try {
      await route.fulfill({
        contentType: "application/json",
        body: execFileSync(fixture, [], {
          input: route.request().postData() ?? "",
          encoding: "utf8",
          maxBuffer: 20 * 1024 * 1024,
          timeout: 30_000,
        }),
      });
    } catch (error) {
      await route.fulfill({
        status: 400,
        body: String((error as { stderr?: unknown }).stderr ?? error),
      });
    }
  });
  await page.goto("/");
  await page.evaluate(async () => {
    const path = "/tests/workflow/sketchHarness.tsx";
    await (await import(/* @vite-ignore */ path)).mount();
  });
  await expect(
    page.getByRole("dialog", { name: "Чертёж и раскрой" }),
  ).toBeVisible();
}
async function number(page: Page, name: string, value: string) {
  const input = page.getByRole("spinbutton", { name, exact: true });
  await input.fill(value);
  await input.press("Tab");
}

test("fan project: native CAM, multiple tools, project roundtrip, stale code and publication", async ({
  page,
}, testInfo) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await mount(page);
  await page.getByRole("button", { name: "Вентилятор", exact: true }).click();
  await page.getByRole("button", { name: "Добавить в чертёж" }).click();
  await expect(page.locator(".sketch-operations button")).toHaveCount(6);
  await page.getByRole("button", { name: /^Панель Деталь/ }).click();
  const select = page.getByLabel("Фреза для фигуры");
  const other = await select.locator("option").allTextContents();
  const label = other.find((text) => text.startsWith("Ø6.35"));
  expect(label).toBeTruthy();
  await select.selectOption({ label: label! });
  await page
    .getByRole("button", { name: "Создать G-code", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Открыть в задании", exact: true }),
  ).toBeEnabled();
  await expect(page.locator(".sketch-feedback")).toContainText(
    "смен инструмента: 1",
  );
  expect(await page.locator(".sketch-cam-path").count()).toBe(6);
  const download = page.waitForEvent("download");
  await page
    .getByRole("button", { name: "Сохранить проект", exact: true })
    .click();
  const saved = await download;
  const path = await saved.path();
  const project = JSON.parse(readFileSync(path!, "utf8"));
  expect(project.version).toBe(1);
  expect(project.document.shapes).toHaveLength(6);
  await number(page, "Толщина листа", "2.5");
  await expect(
    page.getByRole("button", { name: "Открыть в задании", exact: true }),
  ).toBeDisabled();
  await page.getByLabel("Файл проекта Millo").setInputFiles({
    name: "panel.millo-sketch.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(project)),
  });
  await expect(
    page.getByRole("spinbutton", { name: "Толщина листа", exact: true }),
  ).toHaveValue("3");
  await page
    .getByRole("button", { name: "Создать G-code", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Открыть в задании", exact: true }),
  ).toBeEnabled();
  await page.screenshot({
    path: testInfo.outputPath("fan-sketch.png"),
    fullPage: true,
  });
  const overflow = await page
    .locator(".sketch-dialog")
    .evaluate((el) => el.scrollWidth > el.clientWidth + 1);
  expect(overflow).toBe(false);
  await page
    .getByRole("button", { name: "Открыть в задании", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Чертёж и раскрой" }),
  ).not.toBeVisible();
  await expect(page.getByLabel("Published G-code")).toContainText("T2 M6");
  expect(errors).toEqual([]);
});

test("sketch gestures, undo, numeric edits, persistence and invalid project remain usable", async ({
  page,
}, testInfo) => {
  await mount(page);
  await page
    .getByRole("button", { name: "Прямоугольник: два угла", exact: true })
    .click();
  const canvas = page.getByRole("application", { name: "Чертёж заготовки" });
  await canvas.scrollIntoViewIfNeeded();
  const bounds = await canvas.boundingBox();
  expect(bounds).toBeTruthy();
  await page.mouse.move(
    bounds!.x + bounds!.width * 0.35,
    bounds!.y + bounds!.height * 0.35,
  );
  await page.mouse.down();
  await page.mouse.move(
    bounds!.x + bounds!.width * 0.55,
    bounds!.y + bounds!.height * 0.6,
    { steps: 8 },
  );
  await page.mouse.up();
  await expect(page.locator(".sketch-operations button")).toHaveCount(1);
  await number(page, "Ширина фигуры", "40");
  await expect(
    page.getByRole("spinbutton", { name: "Ширина фигуры", exact: true }),
  ).toHaveValue("40");
  await page
    .getByRole("button", { name: "Дублировать фигуру", exact: true })
    .click();
  await expect(page.locator(".sketch-operations button")).toHaveCount(2);
  await page
    .getByRole("button", { name: "Отменить изменение", exact: true })
    .click();
  await expect(page.locator(".sketch-operations button")).toHaveCount(1);
  await page.getByLabel("Файл проекта Millo").setInputFiles({
    name: "bad.json",
    mimeType: "application/json",
    buffer: Buffer.from('{"version":99}'),
  });
  await expect(page.locator(".sketch-feedback")).toContainText(
    "Некорректный формат",
  );
  await expect(page.locator(".sketch-operations button")).toHaveCount(1);
  await expect
    .poll(() =>
      page.evaluate(() => localStorage.getItem("millo.quick-sketch.v1")),
    )
    .toContain('"width":40');
  await page.screenshot({
    path: testInfo.outputPath("rectangle-sketch.png"),
    fullPage: true,
  });
  const before = await canvas.getAttribute("viewBox");
  await canvas.hover();
  await page.mouse.wheel(0, -200);
  await expect(canvas).not.toHaveAttribute("viewBox", before!);
  await page
    .getByRole("button", { name: "Замкнутый контур", exact: true })
    .click();
  await canvas.click({ position: { x: 100, y: 100 } });
  await expect(page.locator(".sketch-polygon-actions")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator(".sketch-polygon-actions")).not.toBeVisible();
  await expect(
    page.getByRole("dialog", { name: "Чертёж и раскрой" }),
  ).toBeVisible();
});
