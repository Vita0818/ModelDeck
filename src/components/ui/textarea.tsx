import * as React from "react";
import { cn } from "@/lib/utils";

const Textarea = React.forwardRef<
  HTMLTextAreaElement,
  React.ComponentProps<"textarea">
>(({ className, ...props }, ref) => (
  <textarea
    ref={ref}
    autoComplete="off"
    autoCorrect="off"
    autoCapitalize="off"
    spellCheck={false}
    className={cn(
      "flex min-h-20 w-full rounded-lg border border-input bg-card px-2.5 py-2 text-base text-foreground",
      "shadow-xs transition-[border-color,box-shadow] duration-150 ease-apple",
      "placeholder:text-muted-foreground/70",
      "focus:border-primary focus:outline-none focus:ring-[3.5px] focus:ring-ring/35",
      "disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  />
));
Textarea.displayName = "Textarea";

export { Textarea };
