import { useEffect, useRef, useState } from "react";
import { fetchAppUpdateNotes, type AppUpdateInfo } from "../lib/api";
import { useI18n } from "../i18n";

type NotesState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "loaded"; markdown: string }
  | { status: "failed" };

export type ReleaseNotesBlock =
  | { kind: "heading"; text: string }
  | { kind: "subheading"; text: string }
  | { kind: "paragraph"; text: string }
  | { kind: "list"; items: string[] };

// Minimal renderer for the project changelog shape: `##`/`###` headings,
// `- ` bullets, blank-line separated paragraphs, and `code` spans. The content
// comes from this project's own release-notes files, so no general Markdown
// support is required.
export function parseReleaseNotesMarkdown(markdown: string): ReleaseNotesBlock[] {
  const blocks: ReleaseNotesBlock[] = [];
  const lines = markdown.split(/\r?\n/);
  let paragraph: string[] = [];
  let list: string[] = [];

  const flushParagraph = () => {
    if (paragraph.length > 0) {
      blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
      paragraph = [];
    }
  };
  const flushList = () => {
    if (list.length > 0) {
      blocks.push({ kind: "list", items: list });
      list = [];
    }
  };

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) {
      flushParagraph();
      flushList();
      continue;
    }

    const heading = /^(#{2,6})\s+(.*)$/.exec(line);
    if (heading) {
      flushParagraph();
      flushList();
      blocks.push({
        kind: heading[1].length <= 2 ? "heading" : "subheading",
        text: heading[2].trim(),
      });
      continue;
    }

    const bullet = /^[-*]\s+(.*)$/.exec(line);
    if (bullet) {
      flushParagraph();
      list.push(bullet[1].trim());
      continue;
    }

    flushList();
    paragraph.push(line);
  }
  flushParagraph();
  flushList();
  return blocks;
}

export function renderInlineText(text: string, keyPrefix: string) {
  const parts = text.split(/(`[^`]+`)/g);
  return parts.map((part, index) =>
    part.startsWith("`") && part.endsWith("`") && part.length > 2 ? (
      <code key={`${keyPrefix}-code-${index}`}>{part.slice(1, -1)}</code>
    ) : (
      part
    ),
  );
}

type AppUpdateNotesProps = {
  info: AppUpdateInfo | null;
};

export function AppUpdateNotes({ info }: AppUpdateNotesProps) {
  const { language, t } = useI18n();
  const [state, setState] = useState<NotesState>({ status: "idle" });
  const [retryCount, setRetryCount] = useState(0);
  const revisionRef = useRef(0);

  const version = info?.available ? info.version : null;
  const notesUrl = info?.available ? info.notesUrl : null;

  useEffect(() => {
    if (!version || !notesUrl) {
      revisionRef.current += 1;
      setState({ status: "idle" });
      return;
    }

    const revision = ++revisionRef.current;
    setState({ status: "loading" });
    fetchAppUpdateNotes(language)
      .then((markdown) => {
        if (revisionRef.current === revision) {
          setState({ status: "loaded", markdown });
        }
      })
      .catch(() => {
        if (revisionRef.current === revision) {
          setState({ status: "failed" });
        }
      });
  }, [version, notesUrl, language, retryCount]);

  if (state.status === "idle") {
    return null;
  }

  return (
    <div className="app-update-notes" data-testid="app-update-notes">
      <h4>{t.appUpdate.notesSection}</h4>
      {state.status === "loading" ? (
        <p data-testid="app-update-notes-loading">{t.appUpdate.notesLoading}</p>
      ) : null}
      {state.status === "failed" ? (
        <div className="app-update-notes__failed" data-testid="app-update-notes-failed">
          <p>{t.appUpdate.notesUnavailable}</p>
          <button
            type="button"
            className="button-secondary button-compact"
            data-testid="app-update-notes-retry"
            onClick={() => setRetryCount((count) => count + 1)}
          >
            {t.appUpdate.notesRetry}
          </button>
        </div>
      ) : null}
      {state.status === "loaded" ? (
        <div className="app-update-notes__body" data-testid="app-update-notes-content">
          {parseReleaseNotesMarkdown(state.markdown).map((block, index) => {
            const key = `notes-block-${index}`;
            switch (block.kind) {
              case "heading":
                return <h5 key={key}>{renderInlineText(block.text, key)}</h5>;
              case "subheading":
                return <h6 key={key}>{renderInlineText(block.text, key)}</h6>;
              case "paragraph":
                return <p key={key}>{renderInlineText(block.text, key)}</p>;
              case "list":
                return (
                  <ul key={key}>
                    {block.items.map((item, itemIndex) => (
                      <li key={`${key}-item-${itemIndex}`}>
                        {renderInlineText(item, `${key}-item-${itemIndex}`)}
                      </li>
                    ))}
                  </ul>
                );
            }
          })}
        </div>
      ) : null}
    </div>
  );
}
