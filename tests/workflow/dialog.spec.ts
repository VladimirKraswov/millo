import { expect, test } from "@playwright/test";

test("dialog stack traps focus, respects live guards and restores its opener", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(async () => {
    const path = "/tests/workflow/dialogHarness.tsx";
    const harness = await import(/* @vite-ignore */ path);
    harness.mount();
  });
  const opener = page.getByRole("button", { name: "Open surface" });
  await opener.click();
  const parent = page.getByRole("dialog", { name: "Parent", exact: true });
  await expect(parent).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(
    page.getByRole("button", { name: "Close parent" }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("textbox", { name: "First input" }),
  ).toBeFocused();
  await page.getByRole("checkbox", { name: "Lock dismissal" }).check();
  await page.keyboard.press("Escape");
  await expect(parent).toBeVisible();
  await page.getByRole("checkbox", { name: "Lock dismissal" }).uncheck();
  const nested = page.getByRole("button", { name: "Nested", exact: true });
  await nested.click();
  await expect(page.getByRole("dialog", { name: "Child" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Child" })).not.toBeVisible();
  await expect(parent).toBeVisible();
  await expect(nested).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(parent).not.toBeVisible();
  await expect(opener).toBeFocused();
  await page.getByRole("button", { name: "Open panel" }).click();
  await expect(parent).toHaveAttribute("aria-modal", "false");
  const outside = page.getByRole("button", { name: "Outside", exact: true });
  await outside.focus();
  await page.keyboard.press("Escape");
  await expect(parent).not.toBeVisible();
  await expect(outside).toBeFocused();
});
