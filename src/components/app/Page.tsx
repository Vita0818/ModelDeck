import { cn } from "@/lib/utils";

/**
 * Content scaffold for every destination: a sticky translucent title bar over
 * a scrolling body, with the body clamped to a comfortable reading width.
 */
export function Page({
  title,
  description,
  actions,
  children,
  bodyClassName,
}: {
  title: React.ReactNode;
  description?: React.ReactNode;
  actions?: React.ReactNode;
  children: React.ReactNode;
  bodyClassName?: string;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header
        className="sticky top-0 z-10 flex shrink-0 items-start gap-4 border-b border-border bg-background/80 px-7 py-4 backdrop-blur-xl"
        data-tauri-drag-region
      >
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-xl font-semibold tracking-tight text-foreground">
            {title}
          </h1>
          {description && (
            <p className="mt-0.5 text-sm text-muted-foreground">
              {description}
            </p>
          )}
        </div>
        {actions && (
          <div className="no-drag flex shrink-0 items-center gap-2 pt-0.5">
            {actions}
          </div>
        )}
      </header>
      <div className={cn("scroll-area flex-1 px-7 py-6", bodyClassName)}>
        <div className="mx-auto w-full max-w-3xl space-y-7">{children}</div>
      </div>
    </div>
  );
}

/** Centered state for empty lists and first-run guidance. */
export function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon?: React.ReactNode;
  title: React.ReactNode;
  description?: React.ReactNode;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-border px-6 py-14 text-center">
      {icon && (
        <div className="grid size-11 place-items-center rounded-full bg-muted text-muted-foreground">
          {icon}
        </div>
      )}
      <div className="space-y-1">
        <p className="text-lg font-medium text-foreground">{title}</p>
        {description && (
          <p className="mx-auto max-w-sm text-sm text-muted-foreground">
            {description}
          </p>
        )}
      </div>
      {action && <div className="pt-1">{action}</div>}
    </div>
  );
}
