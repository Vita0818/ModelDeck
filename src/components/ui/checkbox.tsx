import * as React from "react";
import { Check, Minus } from "lucide-react";
import { cn } from "@/lib/utils";

export interface CheckboxProps
  extends Omit<React.ComponentProps<"input">, "type" | "onChange"> {
  indeterminate?: boolean;
  onChange?: React.ChangeEventHandler<HTMLInputElement>;
  /** Radix-style callback, kept so call sites read the same as the switch. */
  onCheckedChange?: (checked: boolean) => void;
}

/**
 * Checkbox drawn over a native input so form semantics and the indeterminate
 * state stay intact while the visuals follow the accent token.
 */
const Checkbox = React.forwardRef<HTMLInputElement, CheckboxProps>(
  (
    {
      className,
      indeterminate = false,
      checked,
      onChange,
      onCheckedChange,
      ...props
    },
    ref,
  ) => {
    const inner = React.useRef<HTMLInputElement>(null);
    React.useImperativeHandle(ref, () => inner.current as HTMLInputElement);
    React.useLayoutEffect(() => {
      if (inner.current) inner.current.indeterminate = indeterminate;
    }, [indeterminate]);

    const active = indeterminate || Boolean(checked);

    return (
      <span className={cn("relative inline-flex size-[16px]", className)}>
        <input
          ref={inner}
          type="checkbox"
          checked={checked}
          onChange={(event) => {
            onChange?.(event);
            onCheckedChange?.(event.target.checked);
          }}
          className="peer absolute inset-0 cursor-pointer opacity-0 disabled:cursor-not-allowed"
          {...props}
        />
        <span
          aria-hidden
          className={cn(
            "pointer-events-none grid size-full place-items-center rounded-[5px] border transition-colors duration-150 ease-apple",
            "peer-focus-visible:ring-[3.5px] peer-focus-visible:ring-ring/35",
            "peer-disabled:opacity-40",
            active
              ? "border-primary bg-primary text-primary-foreground"
              : "border-input bg-card",
          )}
        >
          {indeterminate ? (
            <Minus className="size-3" strokeWidth={3} />
          ) : checked ? (
            <Check className="size-3" strokeWidth={3} />
          ) : null}
        </span>
      </span>
    );
  },
);
Checkbox.displayName = "Checkbox";

export { Checkbox };
