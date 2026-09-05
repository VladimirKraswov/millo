import { expect, test, type Page } from "@playwright/test";

async function openFixture(page: Page, fixture: string) {
  await page.goto(`/?fixture=${fixture}`);
  await expect(
    page.getByRole("navigation", { name: "Разделы Millo" }),
  ).toBeVisible();
}

test("prepare, confirm, pause, resume and stop without losing the job", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await openFixture(page, "first-cut");
  await page
    .getByRole("button", { name: "Проверить готовность", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Запустить проверку движения", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "Начать движение" });
  const start = dialog.getByRole("button", {
    name: "Начать проверку движения",
    exact: true,
  });
  await expect(dialog).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).toBeVisible();
  await expect(start).toBeDisabled();
  for (const box of await dialog.getByRole("checkbox").all()) await box.check();
  await start.click();
  await expect(dialog).not.toBeVisible();
  const card = page.locator(".program-run-card");
  await expect(card.getByRole("progressbar")).toHaveAttribute(
    "aria-valuenow",
    "25",
  );
  await card.getByRole("button", { name: "Пауза", exact: true }).click();
  await expect(
    card.getByRole("button", { name: "Продолжить", exact: true }),
  ).toBeEnabled();
  const nav = page.getByRole("navigation", { name: "Разделы Millo" });
  await nav.getByRole("button", { name: "Станок", exact: true }).click();
  await nav.getByRole("button", { name: "Задание", exact: true }).click();
  await card.getByRole("button", { name: "Продолжить", exact: true }).click();
  await expect(
    card.getByRole("button", { name: "Пауза", exact: true }),
  ).toBeEnabled();
  await card
    .getByRole("button", { name: "Остановить текущее задание" })
    .click();
  await expect(
    card.getByRole("button", { name: "Подготовить новый запуск" }),
  ).toBeEnabled();
  await expect(
    page.getByRole("button", { name: "Вернуться в рабочий ноль", exact: true }),
  ).toBeVisible();
  expect(errors).toEqual([]);
});

test("a completed check leads back to preparation, not a dead end", async ({
  page,
}) => {
  await openFixture(page, "check-complete");
  await expect(page.locator(".job-readiness")).toBeVisible();
  await expect(page.locator(".job-primary-action")).toBeEnabled();
  await expect(page.locator(".program-run-card")).toHaveCount(0);
});

test("a finished job offers a rerun and does not reopen recovery", async ({
  page,
}) => {
  await openFixture(page, "run-complete");
  await page
    .getByRole("button", { name: "Подготовить повторный запуск" })
    .click();
  await expect(page.locator(".job-readiness")).toBeVisible();
  await expect(page.locator(".program-identity")).toContainText(
    "first-cut-fixture.nc",
  );
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("tool change is explicit and can be reopened after dismissal", async ({
  page,
}) => {
  await openFixture(page, "tool-change");
  const dialog = page.getByRole("dialog", { name: "Установить инструмент T2" });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: "Проверить и продолжить" }),
  ).toBeDisabled();
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await expect(page.locator(".program-run-card")).toContainText("T2");
  await page.getByRole("button", { name: "Подтвердить замену" }).click();
  await dialog.getByRole("checkbox").check();
  await dialog.getByRole("button", { name: "Проверить и продолжить" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(
    page
      .locator(".program-run-card")
      .getByRole("button", { name: "Пауза", exact: true }),
  ).toBeEnabled();
});

test("offline help is searchable and returns keyboard focus", async ({
  page,
}) => {
  await openFixture(page, "first-cut");
  const helpButton = page
    .getByRole("navigation", { name: "Разделы Millo" })
    .getByRole("button", { name: "Справка" });
  await helpButton.click();
  const dialog = page.getByRole("dialog", { name: "Справка по работе" });
  await expect(dialog).toBeVisible();
  const bounds = await dialog.boundingBox();
  expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(
    page.viewportSize()!.height,
  );
  await dialog
    .getByRole("textbox", { name: "Поиск по справке" })
    .fill("пластина");
  await expect(
    dialog.getByRole("heading", { name: "Щуп и карта высот" }),
  ).toBeVisible();
  await dialog
    .getByRole("textbox", { name: "Поиск по справке" })
    .fill("несуществующая тема qqq");
  await expect(
    dialog.getByRole("heading", { name: "Ничего не найдено" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await expect(helpButton).toBeFocused();
});

test("3D scene renders geometry, changes view and stays within the viewport", async ({
  page,
}, testInfo) => {
  await openFixture(page, "first-cut");
  const canvas = page.locator(
    "canvas[aria-label^='Предпросмотр траектории G-code']",
  );
  await expect(canvas).toBeVisible();
  await expect
    .poll(async () => {
      const screenshot = (await canvas.screenshot()).toString("base64");
      return page.evaluate(async (base64) => {
        const image = new Image();
        image.src = `data:image/png;base64,${base64}`;
        await image.decode();
        const decoded = document.createElement("canvas");
        decoded.width = image.width;
        decoded.height = image.height;
        const context = decoded.getContext("2d")!;
        context.drawImage(image, 0, 0);
        const { data } = context.getImageData(0, 0, image.width, image.height);
        const colors = new Set<number>();
        for (let i = 0; i < data.length; i += 4) {
          colors.add((data[i] << 16) | (data[i + 1] << 8) | data[i + 2]);
        }
        return colors.size;
      }, screenshot);
    })
    .toBeGreaterThan(30);
  const initial = await canvas.screenshot();
  await page.locator(".preview-view button").first().click();
  await expect
    .poll(async () => Buffer.compare(initial, await canvas.screenshot()))
    .not.toBe(0);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  const header = await page.locator(".topbar").boundingBox();
  const selector = await page.locator(".machine-switcher").boundingBox();
  expect(selector!.y + selector!.height).toBeLessThanOrEqual(
    header!.y + header!.height,
  );
  if (!testInfo.project.name.includes("mobile")) {
    expect(
      await page.evaluate(() => document.documentElement.scrollHeight),
    ).toBeLessThanOrEqual(page.viewportSize()!.height);
    await expect(page.locator(".job-primary-action")).toBeInViewport();
  }
  const pad = await page.locator(".jog-pad").boundingBox();
  for (const button of await page.locator(".jog-pad-controls button").all()) {
    const bounds = await button.boundingBox();
    expect(bounds!.x).toBeGreaterThanOrEqual(pad!.x);
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(pad!.x + pad!.width);
  }
  await page.screenshot({
    path: testInfo.outputPath("workstation.png"),
    fullPage: true,
  });
});

test("a failed WebGL scene leaves navigation and realtime controls mounted", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const original = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (
      kind: string,
      ...args: unknown[]
    ) {
      if (kind.includes("webgl")) return null;
      return original.apply(this, [kind, ...args] as Parameters<
        typeof original
      >);
    } as typeof original;
  });
  await openFixture(page, "first-cut");
  await expect(
    page.getByRole("heading", { name: /Не удалось показать/ }),
  ).toBeVisible();
  await expect(
    page.getByRole("group", { name: "Остановка станка" }).getByRole("button"),
  ).toHaveCount(3);
  await page
    .getByRole("navigation", { name: "Разделы Millo" })
    .getByRole("button", { name: "Справка" })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Справка по работе" }),
  ).toBeVisible();
});
