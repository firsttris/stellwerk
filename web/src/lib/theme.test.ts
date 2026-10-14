import { describe, expect, it } from "vitest";
import { applyTheme, loadTheme, parseTheme, saveTheme } from "./theme";

describe("theme", () => {
  it("parses what is stored", () => {
    expect(parseTheme("light")).toBe("light");
    expect(parseTheme("dark")).toBe("dark");
    expect(parseTheme("system")).toBe("system");
    expect(parseTheme(null)).toBe("system");
    expect(parseTheme("sepia")).toBe("system");
  });

  it("remembers the choice", () => {
    saveTheme(localStorage, "dark");
    expect(loadTheme(localStorage)).toBe("dark");
    localStorage.clear();
    expect(loadTheme(localStorage)).toBe("system");
  });

  it("follows the system when storage is blocked", () => {
    const blocked = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    } as unknown as Storage;
    expect(() => saveTheme(blocked, "dark")).not.toThrow();
    expect(loadTheme(blocked)).toBe("system");
  });

  it("sets data-theme only for a fixed choice", () => {
    const root = document.createElement("html");
    applyTheme(root, "dark");
    expect(root.dataset.theme).toBe("dark");
    applyTheme(root, "light");
    expect(root.dataset.theme).toBe("light");
    applyTheme(root, "system");
    expect(root.hasAttribute("data-theme")).toBe(false);
  });
});
