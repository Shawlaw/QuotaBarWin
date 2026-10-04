import { beforeAll, describe, expect, test } from "vitest";
import { suppressBrowserContextMenu } from "./contextMenu";

function fireContextMenu(target: Element): boolean {
  const event = new Event("contextmenu", { cancelable: true, bubbles: true });
  target.dispatchEvent(event);
  return event.defaultPrevented;
}

beforeAll(() => {
  suppressBrowserContextMenu(document);
});

describe("suppressBrowserContextMenu", () => {
  test("prevents the browser menu outside editable elements", () => {
    const div = document.createElement("div");
    document.body.appendChild(div);

    expect(fireContextMenu(div)).toBe(true);
  });

  test("keeps the native menu on inputs and textareas", () => {
    const input = document.createElement("input");
    const textarea = document.createElement("textarea");
    document.body.append(input, textarea);

    expect(fireContextMenu(input)).toBe(false);
    expect(fireContextMenu(textarea)).toBe(false);
  });

  test("keeps the menu for editable descendants of blocked areas", () => {
    const wrapper = document.createElement("div");
    const input = document.createElement("input");
    wrapper.appendChild(input);
    document.body.appendChild(wrapper);

    expect(fireContextMenu(input)).toBe(false);
    expect(fireContextMenu(wrapper)).toBe(true);
  });

  test("honors explicit contenteditable state", () => {
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "");
    const disabled = document.createElement("div");
    disabled.setAttribute("contenteditable", "false");
    document.body.append(editable, disabled);

    expect(fireContextMenu(editable)).toBe(false);
    expect(fireContextMenu(disabled)).toBe(true);
  });
});
