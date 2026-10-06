import * as React from "react";
import { cn } from "@/lib/utils";

/**
 * Text field. Apple keeps the resting state almost flat and only reveals the
 * accent on focus, so the border stays hairline-light until then.
 */
export type InputProps = React.ComponentProps<"input">;

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, ...props }, ref) => (
    <input
      ref={ref}
      type={type}
      autoComplete="off"
      autoCorrect="off"
      autoCapitalize="off"
      spellCheck={false}
      className={cn(
        "flex h-8 w-full rounded-lg border border-input bg-card px-2.5 text-base text-foreground",
        "shadow-xs transition-[border-color,box-shadow] duration-150 ease-apple",
        "placeholder:text-muted-foreground/70",
        "focus:border-primary focus:outline-none focus:ring-[3.5px] focus:ring-ring/35",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "file:border-0 file:bg-transparent file:text-base file:font-medium",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";

export { Input };
