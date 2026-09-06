import { expect, test, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { panelProject } from "./sketchFixtures";
import {
  createShape,
  emptySketch,
} from "../../src/plugins/quick-sketch/sketchModel";
import type { SketchOperation } from "../../src/shared/sketch";

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
  const input = page.getByRole("spinbutton", {
    name,
    exact: true,
    includeHidden: true,
  });
  if (!(await input.isVisible())) {
    const ancestors = input.locator("xpath=ancestor::details");
    for (let i = 0; i < (await ancestors.count()); i++) {
      const details = ancestors.nth(i);
      if ((await details.getAttribute("open")) === null)
        await details.locator(":scope > summary").click();
    }
  }
  await input.fill(value);
  await input.press("Tab");
}
async function explorer(page: Page) {
  const toggle = page.getByRole("button", {
    name: "Показать обозреватель",
    exact: true,
  });
  if (await toggle.isVisible()) await toggle.click();
}
async function selectShape(page: Page, name: RegExp) {
  await explorer(page);
  await page
    .locator(".sketch-project-list")
    .getByRole("button", { name })
    .click();
  if (page.viewportSize()!.width <= 1050)
    await page
      .getByRole("button", { name: "Свернуть обозреватель", exact: true })
      .click();
}

test("sheet project: native CAM, multiple tools, project roundtrip, stale code and publication", async ({
  page,
}, testInfo) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await mount(page);
  await expect(
    page.getByRole("button", { name: "Вентилятор", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Решётка", exact: true }),
  ).toHaveCount(0);
  await page.getByLabel("Файл проекта Millo").setInputFiles({
    name: "panel.millo-sketch.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(panelProject())),
  });
  await expect(page.locator(".sketch-figure")).toHaveCount(6);
  await selectShape(page, /^Панель Деталь/);
  await page.getByRole("tab", { name: "Обработка", exact: true }).click();
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
  expect(project.version).toBe(2);
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
    path: testInfo.outputPath("sheet-sketch.png"),
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
  await expect(page.locator(".sketch-figure")).toHaveCount(1);
  await number(page, "Ширина фигуры", "40");
  await expect(
    page.getByRole("spinbutton", { name: "Ширина фигуры", exact: true }),
  ).toHaveValue("40");
  await page
    .getByRole("button", { name: "Дублировать фигуру", exact: true })
    .click();
  await expect(page.locator(".sketch-figure")).toHaveCount(2);
  await page
    .getByRole("button", { name: "Отменить изменение", exact: true })
    .click();
  await expect(page.locator(".sketch-figure")).toHaveCount(1);
  await page.getByLabel("Файл проекта Millo").setInputFiles({
    name: "bad.json",
    mimeType: "application/json",
    buffer: Buffer.from('{"version":99}'),
  });
  await expect(page.locator(".sketch-feedback")).toContainText(
    "Некорректный формат",
  );
  await expect(page.locator(".sketch-figure")).toHaveCount(1);
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

test("dimensions, edge clearance, alignment and drag protection survive project save/load", async ({
  page,
}, testInfo) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await mount(page);
  await page
    .getByLabel("Файл проекта Millo")
    .setInputFiles(
      resolve("fixtures/sketch/constrained-holes.millo-sketch.json"),
    );
  const y = page.getByRole("spinbutton", { name: "Центр Y", exact: true });
  await expect(
    page.getByRole("spinbutton", { name: "Центр X", exact: true }),
  ).toHaveValue("12");
  const drag = async () => {
    const shape = page.locator('[data-shape-id="a"]');
    await shape.scrollIntoViewIfNeeded();
    const b = await shape.boundingBox();
    await page.mouse.move(b!.x + b!.width / 2, b!.y + b!.height / 2);
    await page.mouse.down();
    await page.mouse.move(b!.x + 35, b!.y + 30, { steps: 6 });
    await page.mouse.up();
  };
  await drag();
  await expect(y).toHaveValue("20");
  await page
    .getByRole("button", { name: "Блокировка положения", exact: true })
    .click();
  await page
    .getByRole("button", {
      name: "Разрешить перетаскивание фигур",
      exact: true,
    })
    .click();
  await drag();
  await expect(y).toHaveValue("20");
  await page
    .getByRole("button", { name: "Блокировка положения", exact: true })
    .click();
  await number(page, "Диаметр отверстия", "6");
  await expect(
    page.getByRole("spinbutton", { name: "Центр X", exact: true }),
  ).toHaveValue("13");
  await selectShape(page, /^Отверстие B Карман/);
  await expect(
    page.getByRole("spinbutton", { name: "Центр X", exact: true }),
  ).toHaveValue("43");
  await number(page, "Расстояние X", "25");
  await expect(
    page.getByRole("spinbutton", { name: "Центр X", exact: true }),
  ).toHaveValue("38");
  await expect(page.locator('[data-dimension="X 25 мм"]')).toHaveCount(1);
  await page.getByRole("button", { name: "Ниже опоры", exact: true }).click();
  await number(page, "Расстояние Y", "5");
  await expect(y).toHaveValue("15");
  await page.getByRole("button", { name: "Выше опоры", exact: true }).click();
  await expect(y).toHaveValue("25");
  await page
    .getByRole("button", { name: "Выбирать несколько фигур", exact: true })
    .click();
  await selectShape(page, /^Отверстие A Карман/);
  await page
    .getByRole("button", { name: "По горизонтали", exact: true })
    .click();
  await number(page, "Шаг между центрами", "40");
  await page
    .getByRole("button", { name: "Разместить по X", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Выбирать несколько фигур", exact: true })
    .click();
  await selectShape(page, /^Отверстие A Карман/);
  await number(page, "Центр Y", "30");
  await selectShape(page, /^Отверстие B Карман/);
  await expect(
    page.getByRole("spinbutton", { name: "Центр X", exact: true }),
  ).toHaveValue("53");
  await expect(y).toHaveValue("30");
  await page.screenshot({
    path: testInfo.outputPath("dimensioned-sketch.png"),
    fullPage: true,
  });
  const download = page.waitForEvent("download");
  await page
    .getByRole("button", { name: "Сохранить проект", exact: true })
    .click();
  const saved = await download;
  const data = JSON.parse(readFileSync((await saved.path())!, "utf8"));
  expect(data.document.shapes[1].constraints.x).toMatchObject({
    referenceId: "a",
    offsetMm: 40,
  });
  await page.getByLabel("Файл проекта Millo").setInputFiles({
    name: "roundtrip.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(data)),
  });
  await page
    .getByRole("button", { name: "Создать G-code", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Открыть в задании", exact: true }),
  ).toBeEnabled();
  expect(errors).toEqual([]);
});

test("machining overlays show true tool size and side, stay interactive and reach native CAM", async ({
  page,
}, testInfo) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await mount(page);
  const kinds: SketchOperation["kind"][] = [
    "inside",
    "outside",
    "engrave",
    "pocket",
    "drill",
  ];
  const shapes = kinds.map((kind, i) => {
    const s = createShape(
      { kind: "circle", diameter: kind === "drill" ? 0.8 : 30 },
      25 + i * 37,
      65,
    );
    return {
      ...s,
      id: kind,
      name: kind,
      operation: {
        ...s.operation,
        kind,
        toolId: "preset-carbide3d-102",
        through: false,
        depthMm: 0.5,
        tabs: { ...s.operation.tabs, count: 0 },
      },
    };
  });
  await page.getByLabel("Файл проекта Millo").setInputFiles({
    name: "operations.json",
    mimeType: "application/json",
    buffer: Buffer.from(
      JSON.stringify({ version: 2, document: { ...emptySketch(), shapes } }),
    ),
  });
  await expect(page.locator(".sketch-cutter-footprint")).toHaveCount(4);
  await expect(page.locator(".sketch-pocket-hatch")).toHaveCount(1);
  const centers = await page
    .locator(".sketch-cutter-footprint")
    .evaluateAll((elements) =>
      elements.map((el) => ({
        kind: el.parentElement!.getAttribute("data-operation"),
        x: Number(el.getAttribute("cx")),
        r: Number(el.getAttribute("r")),
      })),
    );
  for (const [index, kind] of kinds.slice(0, 4).entries()) {
    const marker = centers.find((m) => m.kind === kind)!;
    expect(marker.r).toBeCloseTo(3.175 / 2);
    const side = kind === "outside" ? 1 : kind === "engrave" ? 0 : -1;
    expect(marker.x).toBeCloseTo(25 + index * 37 + 15 + side * marker.r);
  }
  await selectShape(page, /^drill /);
  await page.getByRole("tab", { name: "Обработка", exact: true }).click();
  const tools = page.getByLabel("Фреза для фигуры");
  const drill = (await tools.locator("option").allTextContents()).find(
    (label) => label.startsWith("Ø0.8"),
  );
  expect(drill).toBeTruthy();
  await tools.selectOption({ label: drill! });
  await expect(
    page.locator('[data-cutter-for="drill"] circle'),
  ).toHaveAttribute("r", "0.4");
  await selectShape(page, /^inside /);
  const larger = (await tools.locator("option").allTextContents()).find(
    (label) => label.startsWith("Ø6.35"),
  );
  await tools.selectOption({ label: larger! });
  await expect(
    page.locator('[data-cutter-for="inside"] circle'),
  ).toHaveAttribute("r", "3.175");
  await page
    .getByRole("button", { name: "Создать G-code", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Открыть в задании", exact: true }),
  ).toBeEnabled();
  expect(await page.locator(".sketch-cam-path").count()).toBeGreaterThan(4);
  await page
    .getByRole("button", { name: "Показать обработки и фрезы", exact: true })
    .click();
  await expect(page.locator(".sketch-cutter-footprint")).toHaveCount(0);
  await expect(page.locator(".sketch-pocket-hatch")).toHaveCount(0);
  await page
    .getByRole("button", { name: "Показать обработки и фрезы", exact: true })
    .click();
  await page
    .getByRole("application", { name: "Чертёж заготовки" })
    .scrollIntoViewIfNeeded();
  await page.screenshot({
    path: testInfo.outputPath("machining-overlays.png"),
  });
  expect(errors).toEqual([]);
});

test("inline dimensions, explorer actions and expanded editor preserve constraints and undo", async ({
  page,
}, testInfo) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await mount(page);
  await page
    .getByLabel("Файл проекта Millo")
    .setInputFiles(
      resolve("fixtures/sketch/constrained-holes.millo-sketch.json"),
    );
  const diameter = page.getByRole("button", {
    name: "Изменить диаметр",
    exact: true,
  });
  await diameter.hover();
  await expect(diameter.locator("rect")).toHaveCSS("fill", "rgb(24, 61, 75)");
  await diameter.dblclick();
  const input = page.getByRole("textbox", {
    name: "Значение размера",
    exact: true,
  });
  await expect(input).toBeFocused();
  await input.fill("-1");
  await input.press("Enter");
  await expect(page.getByRole("alert")).toContainText("Размер должен быть");
  await page.screenshot({
    path: testInfo.outputPath("inline-size-validation.png"),
  });
  await expect(
    page.getByRole("button", { name: "Создать G-code", exact: true }),
  ).toBeDisabled();
  await input.fill("6,5");
  await input.press("Enter");
  await expect(input).toHaveCount(0);
  await expect(
    page.getByRole("spinbutton", { name: "Диаметр отверстия", exact: true }),
  ).toHaveValue("6.5");
  await selectShape(page, /^Отверстие B Карман/);
  await expect(
    page.getByRole("spinbutton", { name: "Центр X", exact: true }),
  ).toHaveValue("43.25");
  const xDimension = page.getByRole("button", {
    name: "Изменить смещение X",
    exact: true,
  });
  await xDimension.dblclick();
  await input.fill("-5");
  await input.press("Enter");
  await expect(
    page.getByRole("spinbutton", { name: "Центр X", exact: true }),
  ).toHaveValue("8.25");
  await page
    .getByRole("button", { name: "Отменить изменение", exact: true })
    .click();
  await expect(
    page.getByRole("spinbutton", { name: "Центр X", exact: true }),
  ).toHaveValue("43.25");
  await diameter.dblclick();
  await input.fill("100");
  await input.press("Escape");
  await expect(input).toHaveCount(0);
  await expect(
    page.getByRole("dialog", { name: "Чертёж и раскрой" }),
  ).toBeVisible();
  await expect(
    page.getByRole("spinbutton", { name: "Диаметр отверстия", exact: true }),
  ).toHaveValue("4");
  await diameter.dblclick();
  await number(page, "Диаметр отверстия", "5");
  await input.fill("6");
  await input.press("Enter");
  await expect(page.getByRole("alert")).toContainText("Чертёж изменился");
  await input.press("Escape");
  await explorer(page);
  await page
    .getByRole("button", { name: "Переименовать фигуру", exact: true })
    .click();
  const name = page.getByRole("textbox", {
    name: "Новое название фигуры",
    exact: true,
  });
  await name.fill("Монтажное отверстие");
  await name.press("Enter");
  await expect(
    page
      .locator(".sketch-shape-select")
      .filter({ hasText: "Монтажное отверстие" }),
  ).toHaveAttribute("aria-pressed", "true");
  await page
    .getByRole("button", { name: "Создать копию фигуры", exact: true })
    .click();
  await expect(page.locator(".sketch-project-item")).toHaveCount(3);
  await page
    .getByRole("button", {
      name: "Защитить положение выбранных фигур",
      exact: true,
    })
    .click();
  await expect(page.locator(".sketch-figure.is-selected")).toHaveClass(
    /is-locked/,
  );
  await page
    .getByRole("button", { name: "Удалить выбранные фигуры", exact: true })
    .click();
  await expect(page.locator(".sketch-project-item")).toHaveCount(2);
  await page
    .getByRole("button", { name: "Отменить изменение", exact: true })
    .click();
  await expect(page.locator(".sketch-project-item")).toHaveCount(3);
  await page.locator(".sketch-shape-select").first().click();
  await page.locator(".sketch-shape-select").first().press("ArrowDown");
  await expect(page.locator(".sketch-shape-select").nth(1)).toBeFocused();
  await page.locator(".sketch-shape-select").nth(1).press("F2");
  await expect(name).toBeFocused();
  await page
    .getByRole("button", { name: "Свернуть обозреватель", exact: true })
    .click();
  await expect(name).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Создать G-code", exact: true }),
  ).toBeEnabled();
  const canvas = page.getByRole("application", { name: "Чертёж заготовки" });
  const view = await canvas.getAttribute("viewBox");
  await page
    .getByRole("button", { name: "Развернуть редактор", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "Чертёж и раскрой" });
  const box = await dialog.boundingBox();
  expect(box!.width).toBe(page.viewportSize()!.width);
  expect(box!.height).toBe(page.viewportSize()!.height);
  await expect(canvas).toHaveAttribute("viewBox", view!);
  await expect
    .poll(() => dialog.evaluate((el) => el.scrollWidth <= el.clientWidth + 1))
    .toBe(true);
  await canvas.scrollIntoViewIfNeeded();
  await page.screenshot({
    path: testInfo.outputPath("expanded-dimension-editor.png"),
  });
  await page
    .getByRole("button", { name: "Восстановить размер редактора", exact: true })
    .click();
  expect(errors).toEqual([]);
});
