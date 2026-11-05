import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, test, vi } from "vitest";
import { AppUpdateNotes, parseReleaseNotesMarkdown } from "./AppUpdateNotes";
import { I18nProvider } from "../i18n";
import type { AppUpdateInfo } from "../lib/api";

const apiMocks = vi.hoisted(() => ({
  fetchAppUpdateNotes: vi.fn(async () => ""),
}));

vi.mock("../lib/api", () => apiMocks);

function updateInfo(overrides: Partial<AppUpdateInfo> = {}): AppUpdateInfo {
  return {
    configured: true,
    currentVersion: "1.8.0",
    available: true,
    version: "1.9.0",
    notesUrl: "https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/updates/notes/1.9.0.md",
    downloaded: false,
    checkedAt: "2026-11-05T08:00:00Z",
    error: null,
    dismissed: false,
    ...overrides,
  };
}

function renderNotes(info: AppUpdateInfo | null) {
  return render(
    <I18nProvider language="en">
      <AppUpdateNotes info={info} />
    </I18nProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("AppUpdateNotes", () => {
  test("stays hidden until an update with notes is available", () => {
    apiMocks.fetchAppUpdateNotes.mockResolvedValue("## [1.9.0]\n\n- Added something.\n");

    const { rerender } = renderNotes(updateInfo({ available: false, version: null, notesUrl: null }));
    expect(screen.queryByTestId("app-update-notes")).not.toBeInTheDocument();
    expect(apiMocks.fetchAppUpdateNotes).not.toHaveBeenCalled();

    rerender(
      <I18nProvider language="en">
        <AppUpdateNotes info={updateInfo()} />
      </I18nProvider>,
    );

    return waitFor(() => {
      expect(screen.getByTestId("app-update-notes-content")).toBeInTheDocument();
    });
  });

  test("renders the fetched markdown with headings, bullets, and code spans", async () => {
    apiMocks.fetchAppUpdateNotes.mockResolvedValue(
      [
        "## [1.9.0] - 2026-11-05",
        "",
        "### Added",
        "",
        "- Added in-app release notes for `QBWIN_API_KEY` users.",
        "- Second bullet line.",
        "",
        "Closing paragraph.",
      ].join("\n"),
    );

    renderNotes(updateInfo());

    await waitFor(() => {
      expect(screen.getByTestId("app-update-notes-content")).toBeInTheDocument();
    });
    const content = screen.getByTestId("app-update-notes-content");
    expect(content.querySelector("h5")?.textContent).toBe("[1.9.0] - 2026-11-05");
    expect(content.querySelector("h6")?.textContent).toBe("Added");
    expect(content.querySelectorAll("li")).toHaveLength(2);
    expect(content.querySelector("li code")?.textContent).toBe("QBWIN_API_KEY");
    expect(content.textContent).toContain("Closing paragraph.");
    expect(apiMocks.fetchAppUpdateNotes).toHaveBeenCalledWith("en");
  });

  test("shows a failed state with retry after a fetch error", async () => {
    apiMocks.fetchAppUpdateNotes.mockRejectedValueOnce(new Error("offline"));
    apiMocks.fetchAppUpdateNotes.mockResolvedValueOnce("## [1.9.0]\n\n- Added.\n");

    renderNotes(updateInfo());

    await waitFor(() => {
      expect(screen.getByTestId("app-update-notes-failed")).toBeInTheDocument();
    });
    expect(apiMocks.fetchAppUpdateNotes).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByTestId("app-update-notes-retry"));

    await waitFor(() => {
      expect(screen.getByTestId("app-update-notes-content")).toBeInTheDocument();
    });
    expect(apiMocks.fetchAppUpdateNotes).toHaveBeenCalledTimes(2);
  });

  test("ignores a stale response after the offered version changes", async () => {
    let resolveFirst: (markdown: string) => void = () => {};
    apiMocks.fetchAppUpdateNotes.mockImplementationOnce(
      () =>
        new Promise<string>((resolve) => {
          resolveFirst = resolve;
        }),
    );
    apiMocks.fetchAppUpdateNotes.mockResolvedValueOnce("## [2.0.0]\n\n- New.\n");

    const { rerender } = renderNotes(updateInfo());
    rerender(
      <I18nProvider language="en">
        <AppUpdateNotes info={updateInfo({ version: "2.0.0" })} />
      </I18nProvider>,
    );

    // The response for 1.9.0 arrives after the 2.0.0 request started.
    resolveFirst("## [1.9.0]\n\n- Old.\n");
    await waitFor(() => {
      expect(screen.getByTestId("app-update-notes-content").textContent).toContain("2.0.0");
    });
    expect(screen.getByTestId("app-update-notes-content").textContent).not.toContain("Old");
  });
});

describe("parseReleaseNotesMarkdown", () => {
  test("parses the changelog shape into typed blocks", () => {
    const blocks = parseReleaseNotesMarkdown(
      ["## [1.9.0] - 2026-11-05", "", "### Added", "", "- one", "- two", "", "tail"].join("\n"),
    );
    expect(blocks).toEqual([
      { kind: "heading", text: "[1.9.0] - 2026-11-05" },
      { kind: "subheading", text: "Added" },
      { kind: "list", items: ["one", "two"] },
      { kind: "paragraph", text: "tail" },
    ]);
  });

  test("keeps an unclosed list at the end of the document", () => {
    const blocks = parseReleaseNotesMarkdown("### Fixed\n\n- only");
    expect(blocks).toEqual([
      { kind: "subheading", text: "Fixed" },
      { kind: "list", items: ["only"] },
    ]);
  });
});
