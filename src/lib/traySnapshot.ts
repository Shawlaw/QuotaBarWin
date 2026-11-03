import type { ResolvedLanguage } from "../i18n/catalog";
import { copyImageToClipboard } from "./api";
import { visibleAppVersion } from "./appVersion";

const APP_ICON_URL = new URL("../../src-tauri/icons/128x128.png", import.meta.url).href;
const CAPTURE_SCALE = 2;
const OUTER_PADDING = 16;
const HEADER_HEIGHT = 72;
const ICON_SIZE = 42;
const BASE64_CHUNK_SIZE = 0x8000;

type CaptureTrayImageOptions = {
  target: HTMLElement;
  appVersion: string;
  capturedAt: Date;
  capturedAtLabel: (time: string) => string;
  language: ResolvedLanguage;
};

export function formatCaptureTime(date: Date, language: ResolvedLanguage): string {
  return new Intl.DateTimeFormat(language, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function encodeBase64(bytes: Uint8ClampedArray): string {
  const chunks: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += BASE64_CHUNK_SIZE) {
    chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + BASE64_CHUNK_SIZE)));
  }
  return window.btoa(chunks.join(""));
}

function cssColor(variable: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(variable).trim() || fallback;
}

async function loadAppIcon(): Promise<HTMLImageElement> {
  const image = new Image();
  image.src = APP_ICON_URL;
  await image.decode();
  return image;
}

export async function captureTrayImageToClipboard({
  target,
  appVersion,
  capturedAt,
  capturedAtLabel,
  language,
}: CaptureTrayImageOptions): Promise<void> {
  const [content, icon] = await Promise.all([
    import("html2canvas").then(({ default: html2canvas }) => html2canvas(target, {
      backgroundColor: null,
      logging: false,
      scale: CAPTURE_SCALE,
      useCORS: true,
    })),
    loadAppIcon(),
  ]);

  const canvas = document.createElement("canvas");
  canvas.width = content.width + OUTER_PADDING * CAPTURE_SCALE * 2;
  canvas.height = content.height + HEADER_HEIGHT * CAPTURE_SCALE + OUTER_PADDING * CAPTURE_SCALE;
  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("Canvas rendering is unavailable");
  }

  context.fillStyle = cssColor("--qb-canvas", "#f6f8fb");
  context.fillRect(0, 0, canvas.width, canvas.height);

  const padding = OUTER_PADDING * CAPTURE_SCALE;
  const iconSize = ICON_SIZE * CAPTURE_SCALE;
  const textLeft = padding + iconSize + 12 * CAPTURE_SCALE;
  context.drawImage(icon, padding, padding, iconSize, iconSize);

  context.textBaseline = "middle";
  context.fillStyle = cssColor("--qb-text-heading", "#0f172a");
  context.font = `700 ${18 * CAPTURE_SCALE}px "Segoe UI", "Microsoft YaHei", sans-serif`;
  const appName = "QuotaBarWin";
  const titleY = padding + 13 * CAPTURE_SCALE;
  context.fillText(appName, textLeft, titleY);

  const version = visibleAppVersion(appVersion);
  if (version) {
    const nameWidth = context.measureText(appName).width;
    context.fillStyle = cssColor("--qb-text-subtle", "#64748b");
    context.font = `500 ${12 * CAPTURE_SCALE}px "Segoe UI", "Microsoft YaHei", sans-serif`;
    context.fillText(version, textLeft + nameWidth + 10 * CAPTURE_SCALE, titleY);
  }

  context.fillStyle = cssColor("--qb-text-subtle", "#64748b");
  context.font = `400 ${12 * CAPTURE_SCALE}px "Segoe UI", "Microsoft YaHei", sans-serif`;
  context.fillText(
    capturedAtLabel(formatCaptureTime(capturedAt, language)),
    textLeft,
    padding + 34 * CAPTURE_SCALE,
  );

  context.drawImage(content, padding, HEADER_HEIGHT * CAPTURE_SCALE);
  const pixels = context.getImageData(0, 0, canvas.width, canvas.height);
  await copyImageToClipboard({
    width: canvas.width,
    height: canvas.height,
    rgbaBase64: encodeBase64(pixels.data),
  });
}
