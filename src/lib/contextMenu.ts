// The WebView2 default context menu offers browser actions (reload, back,
// print) that reload or navigate the SPA; it is kept only where it carries
// real value: editable fields, whose native menu is cut/copy/paste.
const EDITABLE_SELECTOR =
  'input, textarea, [contenteditable]:not([contenteditable="false"])';

export function suppressBrowserContextMenu(
  document: Document = window.document,
): void {
  document.addEventListener("contextmenu", (event) => {
    const target = event.target;
    if (target instanceof Element && target.closest(EDITABLE_SELECTOR)) {
      return;
    }
    event.preventDefault();
  });
}
