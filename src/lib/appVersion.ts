export function visibleAppVersion(appVersion: string | null | undefined): string | null {
  const version = appVersion?.trim();
  if (!version) {
    return null;
  }

  const label = version.replace(/\([^)]*\)\s*$/, "").trim().replace(/^V/, "v");
  if (!/^v?\d/.test(label)) {
    return null;
  }

  return label.startsWith("v") ? label : `v${label}`;
}
