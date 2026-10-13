import type { FileContent } from "../api/types";

// A line diff for the conflict view: longest common subsequence over a flat typed array. The
// table has (a+1)×(b+1) cells, so both sides are capped; past the cap there is no preview.

export type DiffLine = { kind: "same" | "removed" | "added"; text: string };

/** Lines per side above which no diff is computed: 2 000² cells of 2 bytes are 8 MB. */
export const MAX_DIFF_LINES = 2_000;

/**
 * Lines of `before` and `after` in order, each marked as kept, removed or added; `undefined`
 * when a side has more than `maxLines` lines.
 */
export function diffLines(
  before: string,
  after: string,
  maxLines = MAX_DIFF_LINES,
): DiffLine[] | undefined {
  const a = splitLines(before);
  const b = splitLines(after);
  if (a.length > maxLines || b.length > maxLines) return undefined;
  const width = b.length + 1;
  // lengths[i * width + j]: longest common subsequence of a[i..] and b[j..]; it never exceeds
  // `maxLines`, so 16 bits are enough.
  const lengths = new Uint16Array((a.length + 1) * width);
  const at = (i: number, j: number) => lengths[i * width + j] ?? 0;
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lengths[i * width + j] =
        a[i] === b[j] ? at(i + 1, j + 1) + 1 : Math.max(at(i + 1, j), at(i, j + 1));
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
    } else if (at(i + 1, j) >= at(i, j + 1)) {
      lines.push({ kind: "removed", text: a[i++] ?? "" });
    } else {
      lines.push({ kind: "added", text: b[j++] ?? "" });
    }
  }
  while (i < a.length) lines.push({ kind: "removed", text: a[i++] ?? "" });
  while (j < b.length) lines.push({ kind: "added", text: b[j++] ?? "" });
  return lines;
}

/**
 * The diff a conflict shows: the copy against the current file, or the whole copy as added
 * when the current file was deleted. `undefined` without text on a side that exists.
 */
export function conflictLines(
  original: FileContent | null | undefined,
  copy: FileContent | undefined,
): DiffLine[] | undefined {
  if (copy?.text == null || original === undefined) return undefined;
  if (original === null) return diffLines("", copy.text);
  if (original.text == null) return undefined;
  return diffLines(original.text, copy.text);
}

function splitLines(text: string): string[] {
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}
