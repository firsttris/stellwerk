import { X } from "lucide-react";
import { Dialog as Primitive } from "radix-ui";
import type { ComponentProps, ReactNode } from "react";
import { cn } from "../../lib/utils";
import { m } from "../../paraglide/messages.js";

export const Dialog = Primitive.Root;
export const DialogTrigger = Primitive.Trigger;
export const DialogClose = Primitive.Close;

export function DialogContent({
  className,
  title,
  description,
  children,
  ...props
}: ComponentProps<typeof Primitive.Content> & { title: string; description?: ReactNode }) {
  return (
    <Primitive.Portal>
      <Primitive.Overlay className="fixed inset-0 z-40 bg-black/40" />
      <Primitive.Content
        className={cn(
          "fixed top-1/2 left-1/2 z-50 flex max-h-[calc(100vh-32px)] w-[calc(100vw-32px)] max-w-xl -translate-x-1/2 -translate-y-1/2 flex-col gap-5 overflow-y-auto rounded-xl border border-border bg-background p-6 shadow-xl",
          className,
        )}
        {...props}
      >
        <div className="flex items-start justify-between gap-4">
          <div className="flex flex-col gap-1">
            <Primitive.Title className="text-lg font-semibold">{title}</Primitive.Title>
            {description ? (
              <Primitive.Description className="text-sm text-muted-foreground">
                {description}
              </Primitive.Description>
            ) : (
              <Primitive.Description className="sr-only">{title}</Primitive.Description>
            )}
          </div>
          <Primitive.Close
            className="-mt-1 -mr-2 grid size-10 cursor-pointer place-items-center rounded-md hover:bg-muted"
            aria-label={m.dialog_close()}
          >
            <X className="size-4" />
          </Primitive.Close>
        </div>
        {children}
      </Primitive.Content>
    </Primitive.Portal>
  );
}
