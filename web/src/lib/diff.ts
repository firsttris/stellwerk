// A line diff for the conflict view: longest common subsequence, fine for the small text files
// the Zentrale sends (at most 256 KB).

export type DiffLine = { kind: "same" | "removed" | "added"; text: string };

/** Lines of `before` and `after` in order, each marked as kept, removed or added. */
export function diffLines(before: string, after: string): DiffLine[] {
  const a = splitLines(before);
  const b = splitLines(after);
  // lengths[i][j]: longest common subsequence of a[i..] and b[j..]
  const lengths = Array.from({ length: a.length + 1 }, () =>
    new Array<number>(b.length + 1).fill(0),
  );
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      const row = lengths[i] as number[];
      row[j] =
        a[i] === b[j]
          ? (lengths[i + 1]?.[j + 1] ?? 0) + 1
          : Math.max(lengths[i + 1]?.[j] ?? 0, row[j + 1] ?? 0);
    }
  }
  const lines: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      lines.push({ kind: "same", text: a[i] ?? "" });
      i++;
      j++;
    } else if ((lengths[i + 1]?.[j] ?? 0) >= (lengths[i]?.[j + 1] ?? 0)) {
      lines.push({ kind: "removed", text: a[i++] ?? "" });
    } else {
      lines.push({ kind: "added", text: b[j++] ?? "" });
    }
  }
  while (i < a.length) lines.push({ kind: "removed", text: a[i++] ?? "" });
  while (j < b.length) lines.push({ kind: "added", text: b[j++] ?? "" });
  return lines;
}

function splitLines(text: string): string[] {
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}
