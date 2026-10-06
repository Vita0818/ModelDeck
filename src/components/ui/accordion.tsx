import * as React from "react";
import * as AccordionPrimitive from "@radix-ui/react-accordion";
import { ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";
import { groupSurface } from "@/components/ui/group";

const Accordion = AccordionPrimitive.Root;

const AccordionItem = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Item>
>(({ className, ...props }, ref) => (
  <AccordionPrimitive.Item
    ref={ref}
    className={cn(groupSurface, className)}
    {...props}
  />
));
AccordionItem.displayName = "AccordionItem";

/** Disclosure row. The chevron rotates rather than swapping glyphs. */
const AccordionTrigger = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Trigger>
>(({ className, children, ...props }, ref) => (
  <AccordionPrimitive.Header className="flex">
    <AccordionPrimitive.Trigger
      ref={ref}
      className={cn(
        "flex flex-1 items-center justify-between gap-3 px-3.5 py-3 text-left",
        "transition-colors duration-100 hover:bg-accent",
        "focus-visible:outline-none focus-visible:ring-[3.5px] focus-visible:ring-inset focus-visible:ring-ring/35",
        "[&[data-state=open]>svg]:rotate-180",
        className,
      )}
      {...props}
    >
      {children}
      <ChevronDown className="size-4 shrink-0 text-muted-foreground transition-transform duration-200 ease-apple" />
    </AccordionPrimitive.Trigger>
  </AccordionPrimitive.Header>
));
AccordionTrigger.displayName = AccordionPrimitive.Trigger.displayName;

/** Standard disclosure label: leading glyph, title, and a secondary line. */
function AccordionHeading({
  icon,
  title,
  description,
}: {
  icon?: React.ReactNode;
  title: React.ReactNode;
  description?: React.ReactNode;
}) {
  return (
    <span className="flex min-w-0 items-center gap-3">
      {icon && (
        <span className="grid size-[17px] shrink-0 place-items-center text-muted-foreground [&>svg]:size-[17px]">
          {icon}
        </span>
      )}
      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="text-base font-medium text-foreground">{title}</span>
        {description && (
          <span className="text-sm font-normal text-muted-foreground">
            {description}
          </span>
        )}
      </span>
    </span>
  );
}

const AccordionContent = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Content>
>(({ className, children, ...props }, ref) => (
  <AccordionPrimitive.Content
    ref={ref}
    className="overflow-hidden data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down"
    {...props}
  >
    <div className={cn("border-t border-separator px-3.5 py-3.5", className)}>
      {children}
    </div>
  </AccordionPrimitive.Content>
));
AccordionContent.displayName = AccordionPrimitive.Content.displayName;

export {
  Accordion,
  AccordionItem,
  AccordionTrigger,
  AccordionHeading,
  AccordionContent,
};
