import { captureTrayImageToClipboard, formatCaptureTime } from "./traySnapshot";

const mocks = vi.hoisted(() => ({
  copyImageToClipboard: vi.fn(async () => undefined),
  html2canvas: vi.fn(),
}));

vi.mock("html2canvas", () => ({ default: mocks.html2canvas }));
vi.mock("./api", () => ({ copyImageToClipboard: mocks.copyImageToClipboard }));

afterEach(() => {
  vi.clearAllMocks();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("capture_composes_branding_and_copies_rgba_pixels_without_a_file", async () => {
  const target = document.createElement("section");
  const content = document.createElement("canvas");
  content.width = 200;
  content.height = 100;
  mocks.html2canvas.mockResolvedValue(content);

  const context = {
    drawImage: vi.fn(),
    fillRect: vi.fn(),
    fillStyle: "",
    fillText: vi.fn(),
    font: "",
    getImageData: vi.fn(() => ({ data: new Uint8ClampedArray([1, 2, 3, 4]) })),
    measureText: vi.fn(() => ({ width: 200 })),
    textBaseline: "",
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
    () => context as unknown as CanvasRenderingContext2D,
  );
  const decode = vi.fn(async () => undefined);
  vi.stubGlobal("Image", class {
    src = "";
    decode = decode;
  });

  await captureTrayImageToClipboard({
    target,
    appVersion: "1.6.3(abc1234)",
    capturedAt: new Date("2026-11-03T09:30:00+08:00"),
    capturedAtLabel: (time) => `Captured at ${time}`,
    language: "en",
  });

  expect(mocks.html2canvas).toHaveBeenCalledWith(target, expect.objectContaining({
    backgroundColor: null,
    logging: false,
    scale: 2,
  }));
  expect(decode).toHaveBeenCalledTimes(1);
  expect(context.fillText).toHaveBeenCalledWith("QuotaBarWin", expect.any(Number), expect.any(Number));
  expect(context.fillText).toHaveBeenCalledWith("v1.6.3", expect.any(Number), expect.any(Number));
  expect(context.fillText).toHaveBeenCalledWith(
    expect.stringMatching(/^Captured at /),
    expect.any(Number),
    expect.any(Number),
  );
  expect(context.drawImage).toHaveBeenLastCalledWith(content, 32, 144);
  expect(context.getImageData).toHaveBeenCalledWith(0, 0, 264, 276);
  expect(mocks.copyImageToClipboard).toHaveBeenCalledWith({
    width: 264,
    height: 276,
    rgbaBase64: "AQIDBA==",
  });
});

test("capture_time_uses_seconds_and_the_resolved_language", () => {
  const date = new Date("2026-11-03T09:30:05+08:00");
  const expected = new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);

  expect(formatCaptureTime(date, "zh-CN")).toBe(expected);
});
