import * as React from "react";
import * as SwitchPrimitives from "@radix-ui/react-switch";
import { cn } from "@/lib/utils";

/**
 * Apple's switch: green when on, a soft grey track when off, and a thumb that
 * carries a real contact shadow.
 */
const Switch = React.forwardRef<
  React.ElementRef<typeof SwitchPrimitives.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitives.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitives.Root
    ref={ref}
    className={cn(
      "peer inline-flex h-[26px] w-[42px] shrink-0 cursor-pointer items-center rounded-full",
      "border-2 border-transparent transition-colors duration-200 ease-apple",
      "focus-visible:outline-none focus-visible:ring-[3.5px] focus-visible:ring-ring/35",
      "disabled:cursor-not-allowed disabled:opacity-50",
      "data-[state=checked]:bg-success data-[state=unchecked]:bg-muted",
      "dark:data-[state=unchecked]:bg-muted",
      className,
    )}
    {...props}
  >
    <SwitchPrimitives.Thumb
      className={cn(
        "pointer-events-none block size-[22px] rounded-full bg-white",
        "shadow-[0_1px_3px_rgb(0_0_0/0.25)] ring-0",
        "transition-transform duration-200 ease-spring",
        "data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0",
      )}
    />
  </SwitchPrimitives.Root>
));
Switch.displayName = SwitchPrimitives.Root.displayName;

export { Switch };
