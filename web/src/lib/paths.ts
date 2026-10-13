/** The last part of a path on any system: `C:\Users\demo\Fotos` and `~/Fotos/` give `Fotos`. */
export function baseName(path: string): string {
  return path.split(/[/\\]/).filter(Boolean).at(-1) ?? "";
}
