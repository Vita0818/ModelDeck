import {
  CLIENT_IDS,
  isClientId,
  type ClientId,
} from "@/config/modeldeckClients";

/**
 * Destinations in the shell.
 *
 * The resource layer is the authoritative store, so it sits beside the
 * clients as a peer rather than nested inside one of them. Selecting a client
 * no longer resets which page you are on.
 */
export type ResourceTab = "providers" | "endpoints" | "matrix";

export type Route =
  | { kind: "resources"; tab: ResourceTab }
  | { kind: "client"; client: ClientId }
  | { kind: "usage" }
  | { kind: "settings"; tab?: string }
  | { kind: "hermesMemory" };

const STORAGE_KEY = "modeldeck-route";

export const RESOURCE_TABS: ResourceTab[] = [
  "providers",
  "endpoints",
  "matrix",
];

export function serializeRoute(route: Route): string {
  switch (route.kind) {
    case "resources":
      return `resources:${route.tab}`;
    case "client":
      return `client:${route.client}`;
    case "settings":
      return route.tab ? `settings:${route.tab}` : "settings";
    default:
      return route.kind;
  }
}

export function parseRoute(raw: string | null): Route | null {
  if (!raw) return null;
  const [kind, value] = raw.split(":");
  switch (kind) {
    case "resources":
      return {
        kind: "resources",
        tab: RESOURCE_TABS.includes(value as ResourceTab)
          ? (value as ResourceTab)
          : "providers",
      };
    case "client":
      return isClientId(value) ? { kind: "client", client: value } : null;
    case "usage":
      return { kind: "usage" };
    case "settings":
      return { kind: "settings", tab: value };
    case "hermesMemory":
      return { kind: "hermesMemory" };
    default:
      return null;
  }
}

export function loadRoute(): Route {
  const saved = parseRoute(localStorage.getItem(STORAGE_KEY));
  if (saved) return saved;

  // Carry over the client the previous shell was last showing.
  const legacy = localStorage.getItem("modeldeck-last-client");
  if (isClientId(legacy)) return { kind: "client", client: legacy };
  return { kind: "client", client: CLIENT_IDS[0] as ClientId };
}

export function persistRoute(route: Route) {
  localStorage.setItem(STORAGE_KEY, serializeRoute(route));
}

export function routeEquals(a: Route, b: Route) {
  return serializeRoute(a) === serializeRoute(b);
}
