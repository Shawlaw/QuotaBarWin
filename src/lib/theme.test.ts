import { afterEach, describe, expect, test } from "vitest";
import { applyAppTheme, resolveAppTheme } from "./theme";

afterEach(() => {
  delete document.documentElement.dataset.theme;
});

describe("applyAppTheme", () => {
  test("uses the persisted choice", () => {
    applyAppTheme("dark");

    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
  });

  test("uses light mode when system mode is unavailable", () => {
    applyAppTheme(undefined);

    expect(document.documentElement).toHaveAttribute("data-theme", "light");
  });

  test("keeps explicit selections independent of the system preference", () => {
    expect(resolveAppTheme("light")).toBe("light");
    expect(resolveAppTheme("dark")).toBe("dark");
  });
});
