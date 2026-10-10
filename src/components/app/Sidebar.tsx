import { useTranslation } from "react-i18next";
import {
  BarChart2,
  Boxes,
  Grid2x2Check,
  Monitor,
  Network,
  Settings,
  Terminal,
} from "lucide-react";
import { ProviderIcon } from "@/components/ProviderIcon";
import { cn } from "@/lib/utils";
import type { VisibleApps } from "@/types";
import { MODELDECK_CLIENTS, type ClientId } from "@/config/modeldeckClients";
import { RESOURCE_TABS, routeEquals, type Route } from "./routes";

const RESOURCE_ICONS = {
  providers: Boxes,
  endpoints: Network,
  matrix: Grid2x2Check,
} as const;

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-3 pb-1 pt-4 text-2xs font-semibold uppercase tracking-wider text-muted-foreground/70">
      {children}
    </div>
  );
}

function Item({
  active,
  icon,
  label,
  trailing,
  onClick,
  disabled,
}: {
  active: boolean;
  icon: React.ReactNode;
  label: string;
  trailing?: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-current={active ? "page" : undefined}
      title={label}
      className={cn(
        "group flex w-full items-center gap-2.5 rounded-lg px-2.5 py-[7px] text-left",
        "transition-colors duration-100 ease-apple",
        "focus-visible:outline-none focus-visible:ring-[3.5px] focus-visible:ring-ring/35",
        "disabled:pointer-events-none disabled:opacity-40",
        active
          ? "bg-primary text-primary-foreground shadow-xs"
          : "text-sidebar-foreground hover:bg-black/[0.055] dark:hover:bg-white/[0.07]",
      )}
    >
      <span
        className={cn(
          "grid size-5 shrink-0 place-items-center",
          !active && "text-muted-foreground",
        )}
      >
        {icon}
      </span>
      <span className="min-w-0 flex-1 truncate text-base">{label}</span>
      {trailing}
    </button>
  );
}

/** Small terminal/desktop marker, matching the client badges in the catalog. */
function ClientGlyph({
  client,
}: {
  client: (typeof MODELDECK_CLIENTS)[number];
}) {
  const Badge =
    client.badge === "desktop"
      ? Monitor
      : client.badge === "terminal"
        ? Terminal
        : undefined;
  return (
    <span className="relative inline-flex shrink-0">
      <ProviderIcon icon={client.icon} name={client.name} size={18} />
      {Badge && (
        <span className="absolute -bottom-1 -right-1 grid size-[11px] place-items-center rounded-[3px] bg-sidebar text-muted-foreground">
          <Badge className="size-[7px]" />
        </span>
      )}
    </span>
  );
}

export function Sidebar({
  route,
  onNavigate,
  visibleApps,
  appliedClients,
  width,
}: {
  route: Route;
  onNavigate: (route: Route) => void;
  visibleApps: VisibleApps;
  /** Clients with an applied binding get a dot, like a running indicator. */
  appliedClients: Set<ClientId>;
  width: number;
}) {
  const { t } = useTranslation();
  const clients = MODELDECK_CLIENTS.filter((c) => visibleApps[c.id]);

  return (
    <nav
      aria-label={t("nav.primary")}
      style={{ width }}
      className="material flex shrink-0 flex-col border-r border-border"
      data-testid="sidebar"
    >
      <div className="scroll-area flex-1 px-2 pb-3">
        <SectionLabel>{t("nav.resources")}</SectionLabel>
        {RESOURCE_TABS.map((tab) => {
          const Icon = RESOURCE_ICONS[tab];
          const target: Route = { kind: "resources", tab };
          return (
            <Item
              key={tab}
              active={routeEquals(route, target)}
              icon={<Icon className="size-[17px]" />}
              label={t(`nav.resource.${tab}`)}
              onClick={() => onNavigate(target)}
            />
          );
        })}

        <SectionLabel>{t("nav.clients")}</SectionLabel>
        {clients.map((client) => {
          const target: Route = { kind: "client", client: client.id };
          return (
            <Item
              key={client.id}
              active={routeEquals(route, target)}
              icon={<ClientGlyph client={client} />}
              label={client.name}
              onClick={() => onNavigate(target)}
              trailing={
                appliedClients.has(client.id) ? (
                  <span
                    className="size-1.5 shrink-0 rounded-full bg-success"
                    title={t("nav.applied")}
                  />
                ) : undefined
              }
            />
          );
        })}
      </div>

      <div className="space-y-0.5 border-t border-border px-2 py-2">
        <Item
          active={route.kind === "usage"}
          icon={<BarChart2 className="size-[17px]" />}
          label={t("nav.usage")}
          onClick={() => onNavigate({ kind: "usage" })}
        />
        <Item
          active={route.kind === "settings"}
          icon={<Settings className="size-[17px]" />}
          label={t("common.settings")}
          onClick={() => onNavigate({ kind: "settings" })}
        />
      </div>
    </nav>
  );
}
