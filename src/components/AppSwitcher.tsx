import { useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { VisibleApps } from "@/types";
import { ProviderIcon } from "@/components/ProviderIcon";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { Monitor, MoreHorizontal, Terminal } from "lucide-react";
import {
  CLIENT_IDS,
  MODELDECK_CLIENTS,
  resolveClientVisibility,
  type ClientId,
} from "@/config/modeldeckClients";

interface AppSwitcherProps {
  activeApp: ClientId;
  onSwitch: (app: ClientId) => void;
  visibleApps?: VisibleApps;
  disabled?: boolean;
}

/** 应用图标 + 角标（Claude Code / Desktop 用角标区分终端与桌面） */
function AppGlyph({ app, isActive }: { app: ClientId; isActive: boolean }) {
  const client = MODELDECK_CLIENTS.find((entry) => entry.id === app)!;
  const BadgeIcon =
    client.badge === "desktop"
      ? Monitor
      : client.badge === "terminal"
        ? Terminal
        : undefined;
  return (
    <span className="relative inline-flex shrink-0">
      <ProviderIcon
        icon={client.icon}
        name={MODELDECK_CLIENTS.find((entry) => entry.id === app)!.name}
        size={20}
      />
      {BadgeIcon && (
        <span
          className={cn(
            "absolute -bottom-0.5 -right-0.5 flex items-center justify-center rounded-[3px] border h-[11px] w-[11px]",
            isActive
              ? "bg-background border-border text-foreground"
              : "bg-muted border-background text-muted-foreground group-hover:bg-background group-hover:text-foreground",
          )}
          aria-hidden="true"
        >
          <BadgeIcon className="h-[8px] w-[8px]" strokeWidth={2.5} />
        </span>
      )}
    </span>
  );
}

export function AppSwitcher({
  activeApp,
  onSwitch,
  visibleApps,
  disabled = false,
}: AppSwitcherProps) {
  const { t } = useTranslation();
  const rootRef = useRef<HTMLDivElement>(null);
  const [moreOpen, setMoreOpen] = useState(false);

  const handleSwitch = (app: ClientId) => {
    if (disabled || app === activeApp) return;
    onSwitch(app);
  };

  const visibility = resolveClientVisibility(visibleApps);
  const appsToShow = CLIENT_IDS.filter((app) => visibility[app]);
  const appCount = appsToShow.length;

  const [visibleCount, setVisibleCount] = useState(appCount);

  // 宽度必须取父弹性槽而非自身：自身宽度随可见数量变化，
  // 用它做输入会形成收起→变窄→再收起的反馈循环
  useLayoutEffect(() => {
    const root = rootRef.current;
    const slot = root?.parentElement;
    if (!root || !slot) return;

    const compute = () => {
      const sample = root.querySelector("button");
      if (!sample) return;
      const itemWidth = sample.offsetWidth;
      // jsdom 或未完成布局时 offsetWidth 为 0，保持全部可见
      if (itemWidth <= 0) return;
      const rootStyle = window.getComputedStyle(root);
      const gap = parseFloat(rootStyle.columnGap) || 0;
      const padding =
        (parseFloat(rootStyle.paddingLeft) || 0) +
        (parseFloat(rootStyle.paddingRight) || 0);
      const available = slot.clientWidth;
      const widthAll = padding + appCount * itemWidth + (appCount - 1) * gap;
      if (widthAll <= available) {
        setVisibleCount(appCount);
        return;
      }
      // 「更多」按钮与应用按钮同宽（同 padding + 同尺寸图标）
      const fit = Math.floor(
        (available - padding - itemWidth) / (itemWidth + gap),
      );
      setVisibleCount(Math.max(1, Math.min(appCount - 1, fit)));
    };

    compute();
    const observer = new ResizeObserver(compute);
    observer.observe(slot);
    return () => observer.disconnect();
  }, [appCount]);

  const visibleList = appsToShow.slice(0, Math.max(1, visibleCount));
  // 激活应用被收进溢出区时，顶替最后一个可见位，保证始终可点亮
  if (appsToShow.includes(activeApp) && !visibleList.includes(activeApp)) {
    visibleList[visibleList.length - 1] = activeApp;
  }
  const overflowList = appsToShow.filter((app) => !visibleList.includes(app));

  return (
    <div
      ref={rootRef}
      className="inline-flex bg-muted rounded-xl p-1 gap-1"
      style={{ WebkitAppRegion: "no-drag" } as any}
    >
      {visibleList.map((app) => {
        const isActive = activeApp === app;
        return (
          <button
            key={app}
            type="button"
            disabled={disabled}
            aria-pressed={isActive}
            onClick={() => handleSwitch(app)}
            title={MODELDECK_CLIENTS.find((entry) => entry.id === app)!.name}
            aria-label={
              MODELDECK_CLIENTS.find((entry) => entry.id === app)!.name
            }
            className={cn(
              "group inline-flex items-center px-3 h-8 rounded-md text-sm font-medium",
              isActive
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground hover:bg-background/50",
            )}
          >
            <AppGlyph app={app} isActive={isActive} />
          </button>
        );
      })}
      {overflowList.length > 0 && (
        <Popover open={moreOpen} onOpenChange={setMoreOpen}>
          <PopoverTrigger asChild>
            <button
              type="button"
              disabled={disabled}
              title={t("appSwitcher.more")}
              aria-label={t("appSwitcher.more")}
              className={cn(
                "inline-flex items-center px-3 h-8 rounded-md",
                moreOpen
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground hover:bg-background/50",
              )}
            >
              <MoreHorizontal size={20} className="shrink-0" />
            </button>
          </PopoverTrigger>
          <PopoverContent
            side="bottom"
            align="end"
            sideOffset={6}
            className="z-[100] w-56 p-1"
          >
            {overflowList.map((app) => (
              <button
                key={app}
                type="button"
                disabled={disabled}
                aria-pressed={activeApp === app}
                onClick={() => {
                  setMoreOpen(false);
                  handleSwitch(app);
                }}
                className="group flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm font-medium text-muted-foreground hover:bg-muted hover:text-foreground"
              >
                <AppGlyph app={app} isActive={false} />
                <span className="truncate">
                  {MODELDECK_CLIENTS.find((entry) => entry.id === app)!.name}
                </span>
              </button>
            ))}
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
}
