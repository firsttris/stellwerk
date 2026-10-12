import { type ComponentProps, type ReactNode, useId } from "react";
import { cn } from "../../lib/utils";

export function Input({ className, ...props }: ComponentProps<"input">) {
  return (
    <input
      className={cn(
        "h-10 w-full min-w-0 rounded-md border border-border bg-background px-3 text-sm placeholder:text-muted-foreground disabled:bg-subtle disabled:text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}

export function Select({ className, ...props }: ComponentProps<"select">) {
  return (
    <select
      className={cn(
        "h-10 w-full rounded-md border border-border bg-background px-2.5 text-sm disabled:bg-subtle disabled:text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}

/** A label above its control, linked by id. */
export function Field({ label, children }: { label: string; children: (id: string) => ReactNode }) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-sm font-medium">
        {label}
      </label>
      {children(id)}
    </div>
  );
}
