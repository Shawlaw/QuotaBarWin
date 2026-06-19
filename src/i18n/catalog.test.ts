import { describe, expect, test } from "vitest";
import { createI18n, resolveLanguage } from "./catalog";

describe("i18n language resolution", () => {
  test("keeps_explicit_language_choices", () => {
    expect(resolveLanguage("en", ["zh-CN"])).toBe("en");
    expect(resolveLanguage("zh-CN", ["en-US"])).toBe("zh-CN");
  });

  test("resolves_system_from_browser_languages", () => {
    expect(resolveLanguage("system", ["fr-FR", "zh-Hans-CN", "en-US"])).toBe("zh-CN");
    expect(resolveLanguage("system", ["fr-FR", "en-US"])).toBe("en");
    expect(resolveLanguage("system", ["fr-FR"])).toBe("zh-CN");
  });

  test("creates_catalog_for_resolved_language", () => {
    expect(createI18n("system", ["zh-CN"]).t.header.settings).toBe("设置");
    expect(createI18n().t.header.settings).toBe("设置");
  });
});
