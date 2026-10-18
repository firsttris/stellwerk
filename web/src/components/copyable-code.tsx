import { Copy } from "lucide-react";
import { useRef, useState } from "react";
import { copyToClipboard } from "../lib/clipboard";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";

/** A line of code with a copy button; without a clipboard (plain HTTP) it gets selected. */
export function CopyableCode({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState<"copied" | "selected" | undefined>();
  const codeRef = useRef<HTMLElement>(null);

  const copy = async () => {
    const done = await copyToClipboard(text);
    if (!done && codeRef.current) window.getSelection()?.selectAllChildren(codeRef.current);
    setCopied(done ? "copied" : "selected");
    setTimeout(() => setCopied(undefined), 1800);
  };

  return (
    <div className="flex items-stretch gap-2 rounded-lg border border-border bg-subtle p-2 pl-3">
      <code className="min-w-0 flex-1 self-center font-mono text-[13px] break-all" ref={codeRef}>
        {text}
      </code>
      <Button variant="outline" size="sm" onClick={copy} aria-label={label}>
        <Copy aria-hidden="true" />
        {copied === "copied"
          ? m.pair_copied()
          : copied === "selected"
            ? m.pair_selected()
            : m.pair_copy()}
      </Button>
    </div>
  );
}
