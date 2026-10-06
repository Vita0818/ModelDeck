import * as React from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * Grouped inset list, the primary content shape in macOS System Settings.
 *
 * A `Group` is a rounded card; each `Row` inside it is a full-bleed line with
 * a hairline separator drawn between siblings rather than around them, so the
 * card keeps clean rounded corners.
 */

export function GroupTitle({
  className,
  ...props
}: React.HTMLAttributes<HTMLHeadingElement>) {
  return (
    <h2
      className={cn(
        "px-1 pb-1.5 text-sm font-semibold text-muted-foreground",
        className,
      )}
      {...props}
    />
  );
}

export function Group({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-xl border border-border bg-card shadow-xs",
        "[&>*+*]:border-t [&>*+*]:border-separator",
        className,
      )}
      {...props}
    />
  );
}

export function GroupFooter({
  className,
  ...props
}: React.HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p
      className={cn("px-1 pt-1.5 text-sm text-muted-foreground", className)}
      {...props}
    />
  );
}

export interface RowProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Leading glyph or icon tile. */
  icon?: React.ReactNode;
  label: React.ReactNode;
  /** Secondary line under the label. */
  description?: React.ReactNode;
  /** Trailing control: switch, value text, button. */
  trailing?: React.ReactNode;
  /** Renders the row as a button with a disclosure chevron. */
  onSelect?: () => void;
  disabled?: boolean;
}

export function Row({
  icon,
  label,
  description,
  trailing,
  onSelect,
  disabled,
  className,
  children,
  ...props
}: RowProps) {
  const interactive = Boolean(onSelect);

  const body = (
    <>
      {icon && (
        <span className="grid size-7 shrink-0 place-items-center text-muted-foreground">
          {icon}
        </span>
      )}
      <span className="flex min-w-0 flex-1 flex-col gap-0.5 text-left">
        <span className="truncate text-base text-foreground">{label}</span>
        {description && (
          <span className="text-sm text-muted-foreground">{description}</span>
        )}
      </span>
      {trailing && (
        <span className="flex shrink-0 items-center gap-2 text-base text-muted-foreground">
          {trailing}
        </span>
      )}
      {interactive && (
        <ChevronRight className="size-4 shrink-0 text-muted-foreground/60" />
      )}
    </>
  );

  if (interactive) {
    return (
      <button
        type="button"
        onClick={onSelect}
        disabled={disabled}
        className={cn(
          "flex w-full items-center gap-3 px-3.5 py-2.5 text-left transition-colors duration-100",
          "hover:bg-accent focus-visible:outline-none focus-visible:ring-[3.5px] focus-visible:ring-inset focus-visible:ring-ring/35",
          "disabled:pointer-events-none disabled:opacity-50",
          className,
        )}
      >
        {body}
      </button>
    );
  }

  return (
    <div
      className={cn(
        "flex items-center gap-3 px-3.5 py-2.5",
        disabled && "pointer-events-none opacity-50",
        className,
      )}
      {...props}
    >
      {body}
      {children}
    </div>
  );
}

/** A row whose content is a labelled form control stacked vertically. */
export function FieldRow({
  label,
  hint,
  htmlFor,
  className,
  children,
}: {
  label: React.ReactNode;
  hint?: React.ReactNode;
  htmlFor?: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div className={cn("flex flex-col gap-1.5 px-3.5 py-3", className)}>
      <label
        htmlFor={htmlFor}
        className="text-sm font-medium text-muted-foreground"
      >
        {label}
      </label>
      {children}
      {hint && <p className="text-sm text-muted-foreground">{hint}</p>}
    </div>
  );
}
