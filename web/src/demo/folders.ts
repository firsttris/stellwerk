import { ApiError, type Directory } from "../api/types";

// The demo's file systems for the folder picker: the same few folders in each device's home,
// written the way its system writes paths.

/** `above`: the directories on the way to the home directory, with their one entry. */
type System = { home: string; roots: string[]; sep: "/" | "\\"; above: Record<string, string> };

const systems = {
  windows: {
    home: "C:\\Users\\demo",
    roots: ["C:\\", "D:\\"],
    sep: "\\",
    above: { "C:\\": "Users", "C:\\Users": "demo" },
  },
  macos: { home: "/Users/demo", roots: ["/"], sep: "/", above: { "/": "Users", "/Users": "demo" } },
  linux: { home: "/home/demo", roots: ["/"], sep: "/", above: { "/": "home", "/home": "demo" } },
} satisfies Record<string, System>;

/** Below the home directory, by path relative to it. */
const tree: Record<string, string[]> = {
  "": ["Desktop", "Documents", "Downloads", "Music", "Pictures", "Projects"],
  Pictures: ["Holidays 2026", "Phone"],
  Projects: ["haul", "stellwerk"],
};

const trimSep = (path: string, sep: string) =>
  path.length > 1 && path.endsWith(sep) && !path.endsWith(`:${sep}`) ? path.slice(0, -1) : path;

const join = (dir: string, name: string, sep: string) =>
  dir.endsWith(sep) ? `${dir}${name}` : `${dir}${sep}${name}`;

function parentOf(path: string, system: System): string | null {
  if (system.roots.includes(path)) return null;
  const cut = path.lastIndexOf(system.sep);
  const parent = path.slice(0, cut);
  return system.sep === "/" ? parent || "/" : parent.endsWith(":") ? `${parent}\\` : parent;
}

/** What the device's agent would list at `requested`. */
export function demoDirectory(os: string, requested: string): Directory {
  const system: System = os in systems ? systems[os as keyof typeof systems] : systems.linux;
  const path = trimSep(
    requested.trim().replace(/^~(?=$|[/\\])/, system.home) || system.home,
    system.sep,
  );
  if (!system.roots.some((root) => path.startsWith(root)))
    throw new ApiError(400, `not an absolute path: ${requested}`);
  const below = path.startsWith(system.home) ? path.slice(system.home.length + 1) : undefined;
  const onTheWay = system.above[path];
  const names = below !== undefined ? (tree[below] ?? []) : onTheWay ? [onTheWay] : [];
  return {
    path,
    parent: parentOf(path, system),
    dirs: names.map((name) => ({ name, path: join(path, name, system.sep) })),
    home: system.home,
    roots: system.roots,
  };
}
