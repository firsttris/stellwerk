import { describe, expect, it } from "vitest";
import { diffLines } from "./diff";

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
});
