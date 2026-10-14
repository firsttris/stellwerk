/** Light, dark, or whatever the operating system says. */
export type Theme = "system" | "light" | "dark";

export const themes: readonly Theme[] = ["system", "light", "dark"];

const key = "stellwerk-theme";

/** Anything unknown, including nothing stored yet, means "follow the system". */
export function parseTheme(value: string | null): Theme {
  return themes.find((theme) => theme === value) ?? "system";
}

// Storage can throw (blocked site data); the theme is a convenience, so that is not an error.
export function loadTheme(storage: Storage): Theme {
  try {
    return parseTheme(storage.getItem(key));
  } catch {
    return "system";
  }
}

export function saveTheme(storage: Storage, theme: Theme): void {
  try {
    storage.setItem(key, theme);
  } catch {}
}

/** `styles.css` picks the tokens from `data-theme`; without it the system's scheme decides. */
export function applyTheme(root: HTMLElement, theme: Theme): void {
  if (theme === "system") root.removeAttribute("data-theme");
  else root.dataset.theme = theme;
}
