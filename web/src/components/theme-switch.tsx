import { Monitor, Moon, Sun } from "lucide-react";
import { useState } from "react";
import { applyTheme, loadTheme, saveTheme, type Theme } from "../lib/theme";
import { cn } from "../lib/utils";
import { m } from "../paraglide/messages.js";

const options: { theme: Theme; label: () => string; Icon: typeof Sun }[] = [
  { theme: "system", label: m.theme_system, Icon: Monitor },
  { theme: "light", label: m.theme_light, Icon: Sun },
  { theme: "dark", label: m.theme_dark, Icon: Moon },
];

/** System, light or dark; kept in the browser like the language. */
export function ThemeSwitch() {
  const [theme, setTheme] = useState(() => loadTheme(localStorage));

  const choose = (next: Theme) => {
    setTheme(next);
    saveTheme(localStorage, next);
    applyTheme(document.documentElement, next);
  };

  return (
    <fieldset className="flex gap-0.5 rounded-md border border-border bg-background p-0.5">
      <legend className="sr-only">{m.theme()}</legend>
      {options.map(({ theme: option, label, Icon }) => (
        <label
          key={option}
          className={cn(
            "flex h-8 flex-1 cursor-pointer items-center justify-center gap-1.5 rounded text-xs text-muted-foreground hover:text-foreground has-focus-visible:outline-2 has-focus-visible:outline-primary",
            theme === option && "bg-muted font-medium text-foreground",
          )}
        >
          <input
            type="radio"
            name="theme"
            value={option}
            checked={theme === option}
            onChange={() => choose(option)}
            className="sr-only"
          />
          <Icon className="size-3.5" aria-hidden="true" />
          {label()}
        </label>
      ))}
    </fieldset>
  );
}
