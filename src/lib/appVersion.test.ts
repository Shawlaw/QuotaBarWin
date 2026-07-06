import { visibleAppVersion } from "./appVersion";

test("visible_app_version_keeps_commit_for_hover_only", () => {
  expect(visibleAppVersion("1.0.0(abc1234)")).toBe("v1.0.0");
  expect(visibleAppVersion("v1.2.3")).toBe("v1.2.3");
});

test("visible_app_version_ignores_non_release_labels", () => {
  expect(visibleAppVersion("browser-preview")).toBeNull();
  expect(visibleAppVersion("unknown")).toBeNull();
  expect(visibleAppVersion(null)).toBeNull();
});
