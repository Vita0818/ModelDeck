import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  Copy,
  Eye,
  EyeOff,
  Loader2,
  Network,
  Plus,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Group,
  GroupFooter,
  GroupTitle,
  FieldRow,
  Row,
} from "@/components/ui/group";
import { Page, EmptyState } from "@/components/app/Page";
import { PROTOCOLS, PROTOCOL_LABELS } from "@/lib/api/deck";
import { copyText } from "@/lib/clipboard";
import { extractErrorMessage } from "@/utils/errorUtils";
import { newId, useResourceDraft } from "./useResourceDraft";
import { ResourceToolbar } from "./ResourceToolbar";

export default function EndpointsPage() {
  const { t } = useTranslation();
  const state = useResourceDraft();
  const { draft, setDraft } = state;
  const [selectedId, setSelectedId] = useState("");
  const [revealToken, setRevealToken] = useState(false);
  const [aliasDraft, setAliasDraft] = useState("");

  const endpoints = useMemo(
    () => Object.values(draft?.endpoints ?? {}),
    [draft],
  );
  const endpoint =
    (selectedId ? draft?.endpoints[selectedId] : undefined) ?? endpoints[0];
  const models = useMemo(() => Object.values(draft?.models ?? {}), [draft]);

  if (state.isLoading) {
    return (
      <Page title={t("nav.resource.endpoints")}>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          {t("deck.loading")}
        </div>
      </Page>
    );
  }
  if (state.error || !draft) {
    return (
      <Page title={t("nav.resource.endpoints")}>
        <EmptyState
          title={t("deck.loadFailed")}
          description={extractErrorMessage(state.error)}
          action={
            <Button variant="outline" onClick={() => void state.refetch()}>
              {t("deck.reload")}
            </Button>
          }
        />
      </Page>
    );
  }

  const patchEndpoint = (patch: Partial<NonNullable<typeof endpoint>>) => {
    if (!endpoint) return;
    setDraft({
      ...draft,
      endpoints: {
        ...draft.endpoints,
        [endpoint.id]: { ...endpoint, ...patch },
      },
    });
  };

  const addEndpoint = () => {
    const id = newId();
    setDraft({
      ...draft,
      endpoints: {
        ...draft.endpoints,
        [id]: {
          id,
          name: t("deck.newEndpoint"),
          protocols: ["openai-chat"],
          publicUrl: "",
          token: "",
          routes: {},
        },
      },
    });
    setSelectedId(id);
  };

  const deleteEndpoint = () => {
    if (!endpoint) return;
    const used = Object.values(draft.bindings).some(
      (b) => b?.endpointId === endpoint.id,
    );
    if (used) {
      toast.error(t("deck.endpointInUse"));
      return;
    }
    const next = { ...draft.endpoints };
    delete next[endpoint.id];
    setDraft({ ...draft, endpoints: next });
    setSelectedId("");
  };

  const addRoute = () => {
    if (!endpoint) return;
    const alias = aliasDraft.trim();
    if (!alias) return;
    if (endpoint.routes[alias]) {
      toast.error(t("deck.aliasExists"));
      return;
    }
    patchEndpoint({
      routes: {
        ...endpoint.routes,
        [alias]: { modelId: models[0]?.id ?? "", effort: "" },
      },
    });
    setAliasDraft("");
  };

  const removeRoute = (alias: string) => {
    if (!endpoint) return;
    const routes = { ...endpoint.routes };
    delete routes[alias];
    patchEndpoint({ routes });
  };

  if (!endpoint) {
    return (
      <Page
        title={t("nav.resource.endpoints")}
        description={t("deck.endpointsDescription")}
      >
        <ResourceToolbar {...state} />
        <EmptyState
          icon={<Network className="size-5" />}
          title={t("deck.addFirstEndpoint")}
          description={t("deck.addFirstEndpointHint")}
          action={
            <Button onClick={addEndpoint}>
              <Plus />
              {t("deck.addEndpoint")}
            </Button>
          }
        />
      </Page>
    );
  }

  const routes = Object.entries(endpoint.routes);

  return (
    <Page
      title={t("nav.resource.endpoints")}
      description={t("deck.endpointsDescription")}
      actions={
        <Button variant="outline" onClick={addEndpoint}>
          <Plus />
          {t("deck.addEndpoint")}
        </Button>
      }
    >
      <ResourceToolbar {...state} />

      <section>
        <GroupTitle>{t("deck.endpoints")}</GroupTitle>
        <Group>
          {endpoints.map((e) => (
            <Row
              key={e.id}
              label={e.name}
              description={t("deck.routeCount", {
                count: Object.keys(e.routes).length,
              })}
              trailing={
                <div className="flex gap-1">
                  {e.protocols.map((p) => (
                    <Badge key={p} variant="secondary">
                      {PROTOCOL_LABELS[p].split(" ")[0]}
                    </Badge>
                  ))}
                </div>
              }
              onSelect={() => {
                setSelectedId(e.id);
                setRevealToken(false);
              }}
              className={e.id === endpoint.id ? "bg-primary/[0.07]" : undefined}
            />
          ))}
        </Group>
      </section>

      <section>
        <div className="flex items-end justify-between">
          <GroupTitle>{endpoint.name}</GroupTitle>
          <Button
            variant="ghost"
            size="sm"
            className="mb-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive"
            onClick={deleteEndpoint}
          >
            <Trash2 />
            {t("deck.deleteEndpoint")}
          </Button>
        </div>
        <Group>
          <FieldRow label={t("deck.endpointName")}>
            <Input
              value={endpoint.name}
              onChange={(e) => patchEndpoint({ name: e.target.value })}
            />
          </FieldRow>
          <FieldRow
            label={t("deck.inboundProtocols")}
            hint={t("deck.inboundProtocolsHint")}
          >
            <div className="flex flex-wrap gap-x-5 gap-y-2 pt-0.5">
              {PROTOCOLS.map((p) => (
                <label key={p} className="flex items-center gap-2 text-base">
                  <Checkbox
                    checked={endpoint.protocols.includes(p)}
                    onCheckedChange={(checked) =>
                      patchEndpoint({
                        protocols: checked
                          ? [...endpoint.protocols, p]
                          : endpoint.protocols.filter((x) => x !== p),
                      })
                    }
                  />
                  {PROTOCOL_LABELS[p]}
                </label>
              ))}
            </div>
          </FieldRow>
          <FieldRow label={t("deck.publicUrl")} hint={t("deck.publicUrlHint")}>
            <Input
              value={endpoint.publicUrl}
              onChange={(e) => patchEndpoint({ publicUrl: e.target.value })}
              placeholder={`https://gateway.example/deck/${endpoint.id}`}
            />
          </FieldRow>
          {endpoint.token && (
            <FieldRow
              label={t("deck.accessToken")}
              hint={t("deck.accessTokenHint")}
            >
              <div className="relative">
                <Input
                  readOnly
                  type={revealToken ? "text" : "password"}
                  value={endpoint.token}
                  className="pr-[4.5rem] font-mono"
                />
                <div className="absolute right-0.5 top-0.5 flex">
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t(revealToken ? "common.hide" : "common.show")}
                    onClick={() => setRevealToken((v) => !v)}
                  >
                    {revealToken ? <EyeOff /> : <Eye />}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t("common.copy")}
                    onClick={() => {
                      void copyText(endpoint.token);
                      toast.success(t("common.copied"));
                    }}
                  >
                    <Copy />
                  </Button>
                </div>
              </div>
            </FieldRow>
          )}
        </Group>
      </section>

      <section>
        <GroupTitle>{t("deck.routes")}</GroupTitle>
        <Group>
          {routes.map(([alias, route]) => (
            <div
              key={alias}
              className="flex items-center gap-2.5 px-3.5 py-2.5"
            >
              <code className="w-36 shrink-0 truncate font-mono text-base text-foreground">
                {alias}
              </code>
              <Select
                value={route.modelId || undefined}
                onValueChange={(value) =>
                  patchEndpoint({
                    routes: {
                      ...endpoint.routes,
                      [alias]: { ...route, modelId: value },
                    },
                  })
                }
              >
                <SelectTrigger className="flex-1">
                  <SelectValue placeholder={t("deck.selectModel")} />
                </SelectTrigger>
                <SelectContent>
                  {models.map((m) => (
                    <SelectItem key={m.id} value={m.id}>
                      {m.name} · {m.upstreamId}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Input
                value={route.effort}
                onChange={(e) =>
                  patchEndpoint({
                    routes: {
                      ...endpoint.routes,
                      [alias]: { ...route, effort: e.target.value },
                    },
                  })
                }
                placeholder={t("deck.providerDefault")}
                className="w-28 shrink-0"
              />
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={t("deck.removeRoute")}
                onClick={() => removeRoute(alias)}
                className="text-muted-foreground hover:text-destructive"
              >
                <Trash2 />
              </Button>
            </div>
          ))}
          <div className="flex items-center gap-2.5 px-3.5 py-2.5">
            <Input
              value={aliasDraft}
              onChange={(e) => setAliasDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addRoute();
                }
              }}
              placeholder={t("deck.newAlias")}
              aria-label={t("deck.newAlias")}
              className="w-36 shrink-0 font-mono"
            />
            <Button
              variant="outline"
              size="sm"
              onClick={addRoute}
              disabled={!aliasDraft.trim()}
            >
              <Plus />
              {t("deck.addRoute")}
            </Button>
          </div>
        </Group>
        <GroupFooter>{t("deck.routesHint")}</GroupFooter>
      </section>
    </Page>
  );
}
