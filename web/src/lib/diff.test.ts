import { describe, expect, it } from "vitest";
import { conflictLines, diffLines } from "./diff";

describe("diffLines", () => {
  it("marks kept, removed and added lines in order", () => {
    expect(diffLines("a\nb\nc\n", "a\nx\nc\nd\n")).toEqual([
      { kind: "same", text: "a" },
      { kind: "removed", text: "b" },
      { kind: "added", text: "x" },
      { kind: "same", text: "c" },
      { kind: "added", text: "d" },
    ]);
  });

  it("handles empty sides", () => {
    expect(diffLines("", "a")).toEqual([{ kind: "added", text: "a" }]);
    expect(diffLines("a\n", "")).toEqual([{ kind: "removed", text: "a" }]);
    expect(diffLines("same", "same")).toEqual([{ kind: "same", text: "same" }]);
  });

  it("gives up above the line cap instead of building a huge table", () => {
    const lines = (n: number) => Array.from({ length: n }, (_, i) => `line ${i}`).join("\n");
    expect(diffLines(lines(3), lines(4), 3)).toBeUndefined();
    expect(diffLines(lines(3), lines(3), 3)).toHaveLength(3);
    expect(diffLines(lines(2_001), "")).toBeUndefined();
  });

  it("diffs the largest allowed sides", () => {
    const side = (prefix: string) =>
      Array.from({ length: 2_000 }, (_, i) => `${i % 3 === 0 ? prefix : ""}${i}`).join("\n");
    const lines = diffLines(side("a"), side("b"));
    expect(lines?.filter((l) => l.kind === "same")).toHaveLength(1_333);
  });
});

describe("conflictLines", () => {
  const file = (text: string | null) => ({ size: 1, modified: 0, text });

  it("diffs the copy against the current file", () => {
    expect(conflictLines(file("a\n"), file("b\n"))).toEqual([
      { kind: "removed", text: "a" },
      { kind: "added", text: "b" },
    ]);
  });

  it("shows the whole copy when the current file was deleted", () => {
    expect(conflictLines(null, file("a\nb\n"))).toEqual([
      { kind: "added", text: "a" },
      { kind: "added", text: "b" },
    ]);
  });

  it("has no preview without text or while loading", () => {
    expect(conflictLines(file(null), file("a"))).toBeUndefined();
    expect(conflictLines(file("a"), file(null))).toBeUndefined();
    expect(conflictLines(undefined, undefined)).toBeUndefined();
  });
});
