import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  CircleCheck,
  Copy,
  Loader2,
  Network,
  RotateCcw,
  Save,
  Zap,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
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
} from "@/components/ui/group";
import { Page, EmptyState } from "@/components/app/Page";
import { copyText } from "@/lib/clipboard";
import { extractErrorMessage } from "@/utils/errorUtils";
import {
  useApplyBinding,
  useDeck,
  useRestoreBinding,
  useSaveDeck,
} from "@/lib/query/deck";
import type { HarnessBinding } from "@/lib/api/deck";
import { MODELDECK_CLIENTS, type ClientId } from "@/config/modeldeckClients";
import type { Route } from "@/components/app/routes";

/** Clients launched through a managed command can override the executable. */
const EXECUTABLE_CLIENTS: ClientId[] = [
  "claude",
  "antigravity",
  "muse",
  "cursor-agent",
  "dsh",
  "copilot",
];

const blank = (): HarnessBinding => ({
  endpointId: "",
  model: "",
  utilityModel: "",
  executable: "",
});

export default function HarnessPage({
  client,
  onNavigate,
}: {
  client: ClientId;
  onNavigate: (route: Route) => void;
}) {
  const { t } = useTranslation();
  const { data: view, isLoading, error, refetch } = useDeck();
  const save = useSaveDeck();
  const apply = useApplyBinding();
  const restore = useRestoreBinding();

  const [draft, setDraft] = useState<HarnessBinding>(blank);
  const [command, setCommand] = useState("");

  const saved = view?.store.bindings[client];
  const active = view?.store.activeBindings[client];

  // Re-seed the form whenever the client or the persisted binding changes.
  useEffect(() => {
    setDraft(saved ? { ...saved } : blank());
    setCommand("");
  }, [client, saved]);

  const meta = MODELDECK_CLIENTS.find((c) => c.id === client);
  const endpoints = useMemo(
    () => Object.values(view?.store.endpoints ?? {}),
    [view],
  );
  const compatibility = view?.compatibility.filter((c) => c.client === client);
  const selected = compatibility?.find(
    (c) => c.endpointId === draft.endpointId,
  );
  const models = selected?.models ?? [];
  const selectedRoute =
    view?.store.endpoints[draft.endpointId]?.routes[draft.model];
  const selectedModel = selectedRoute
    ? view?.store.models[selectedRoute.modelId]
    : undefined;
  const busy =
    save.isPending || apply.isPending || restore.isPending || isLoading;

  const patch = (next: Partial<HarnessBinding>) =>
    setDraft((prev) => ({ ...prev, ...next }));

  const persist = async () => {
    if (!view) throw new Error(t("deck.loading"));
    const bindings = { ...view.store.bindings };
    if (draft.endpointId) bindings[client] = draft;
    else delete bindings[client];
    return save.mutateAsync({ ...view.store, bindings });
  };

  const onSave = async () => {
    try {
      await persist();
      toast.success(t("deck.selectionSaved"));
    } catch (e) {
      toast.error(extractErrorMessage(e));
    }
  };

  const onApply = async () => {
    try {
      const next = await persist();
      const result = await apply.mutateAsync({
        client,
        revision: next.store.revision,
      });
      setCommand(result.command ?? "");
      toast.success(
        t(result.restartRequired ? "deck.appliedRestart" : "deck.applied"),
      );
    } catch (e) {
      toast.error(extractErrorMessage(e));
    }
  };

  const onRestore = async () => {
    if (!view) return;
    try {
      await restore.mutateAsync({ client, revision: view.store.revision });
      setCommand("");
      toast.success(t("deck.restored"));
    } catch (e) {
      toast.error(extractErrorMessage(e));
    }
  };

  if (isLoading) {
    return (
      <Page title={meta?.name ?? client}>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          {t("deck.loading")}
        </div>
      </Page>
    );
  }

  if (error || !view) {
    return (
      <Page title={meta?.name ?? client}>
        <EmptyState
          title={t("deck.loadFailed")}
          description={extractErrorMessage(error)}
          action={
            <Button variant="outline" onClick={() => void refetch()}>
              {t("deck.reload")}
            </Button>
          }
        />
      </Page>
    );
  }

  if (!endpoints.length) {
    return (
      <Page title={meta?.name ?? client}>
        <EmptyState
          icon={<Network className="size-5" />}
          title={t("deck.noEndpoints")}
          description={t("deck.noEndpointsHint")}
          action={
            <Button
              onClick={() =>
                onNavigate({ kind: "resources", tab: "endpoints" })
              }
            >
              {t("deck.goToResources")}
            </Button>
          }
        />
      </Page>
    );
  }

  const dirty = JSON.stringify(draft) !== JSON.stringify(saved ?? blank());
  const launchCommand = command || (active ? view.commands?.[client] : "");

  return (
    <Page
      title={meta?.name ?? client}
      description={
        active
          ? t("deck.activeBinding", { model: active.model })
          : t("deck.notApplied")
      }
      actions={
        <>
          {active && (
            <Button variant="ghost" onClick={onRestore} disabled={busy}>
              <RotateCcw />
              {t("deck.restoreNative")}
            </Button>
          )}
          <Button variant="outline" onClick={onSave} disabled={busy || !dirty}>
            <Save />
            {t("deck.saveSelection")}
          </Button>
          <Button onClick={onApply} disabled={busy || !draft.endpointId}>
            {apply.isPending ? <Loader2 className="animate-spin" /> : <Zap />}
            {t("deck.saveAndApply")}
          </Button>
        </>
      }
    >
      <section>
        <GroupTitle>{t("deck.connection")}</GroupTitle>
        <Group>
          <FieldRow label={t("deck.endpoint")}>
            <Select
              value={draft.endpointId}
              onValueChange={(value) =>
                patch({ endpointId: value, model: "", utilityModel: "" })
              }
              disabled={busy}
            >
              <SelectTrigger>
                <SelectValue placeholder={t("deck.selectEndpoint")} />
              </SelectTrigger>
              <SelectContent>
                {endpoints.map((endpoint) => {
                  const compat = compatibility?.find(
                    (c) => c.endpointId === endpoint.id,
                  );
                  return (
                    <SelectItem key={endpoint.id} value={endpoint.id}>
                      {endpoint.name}
                      {compat?.issue ? ` — ${compat.issue}` : ""}
                    </SelectItem>
                  );
                })}
              </SelectContent>
            </Select>
          </FieldRow>

          <FieldRow
            label={t("deck.defaultModel")}
            hint={
              draft.endpointId && !models.length
                ? t("deck.noCompatibleModels")
                : undefined
            }
          >
            <Select
              value={draft.model}
              onValueChange={(value) => patch({ model: value })}
              disabled={busy || !models.length}
            >
              <SelectTrigger>
                <SelectValue placeholder={t("deck.selectModel")} />
              </SelectTrigger>
              <SelectContent>
                {models.map((model) => (
                  <SelectItem key={model} value={model}>
                    {model}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FieldRow>

          {selectedModel && (
            <div className="space-y-2 px-3.5 py-3" data-testid="model-settings">
              <dl className="grid gap-3 sm:grid-cols-3">
                <div>
                  <dt className="text-sm text-muted-foreground">
                    {t("deck.context")}
                  </dt>
                  <dd className="font-mono text-base tabular-nums">
                    {selectedModel.context.toLocaleString()} tokens
                  </dd>
                </div>
                <div>
                  <dt className="text-sm text-muted-foreground">
                    {t("deck.maxOutput")}
                  </dt>
                  <dd className="font-mono text-base tabular-nums">
                    {selectedModel.output.toLocaleString()} tokens
                  </dd>
                </div>
                <div>
                  <dt className="text-sm text-muted-foreground">
                    {t("deck.effort")}
                  </dt>
                  <dd className="text-base">
                    {selectedRoute?.effort || t("deck.providerDefault")}
                  </dd>
                </div>
              </dl>
              <p className="text-sm text-muted-foreground">
                {t("deck.modelSettingsHint")}
              </p>
            </div>
          )}

          {!client.startsWith("copilot") && (
            <FieldRow
              label={t("deck.utilityModel")}
              hint={t("deck.utilityModelHint")}
            >
              <Select
                value={draft.utilityModel || "__none__"}
                onValueChange={(value) =>
                  patch({ utilityModel: value === "__none__" ? "" : value })
                }
                disabled={busy || !models.length}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="__none__">
                    {t("deck.sameAsDefault")}
                  </SelectItem>
                  {models.map((model) => (
                    <SelectItem key={model} value={model}>
                      {model}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </FieldRow>
          )}

          {EXECUTABLE_CLIENTS.includes(client) && (
            <FieldRow
              label={t("deck.executable")}
              hint={t("deck.executableHint")}
            >
              <Input
                value={draft.executable ?? ""}
                onChange={(e) => patch({ executable: e.target.value })}
                placeholder={t("deck.executablePlaceholder")}
                disabled={busy}
              />
            </FieldRow>
          )}
        </Group>
        {selected?.issue && (
          <GroupFooter className="text-warning">{selected.issue}</GroupFooter>
        )}
        {client === "copilot-desktop" && (
          <GroupFooter>{t("deck.copilotDesktopHint")}</GroupFooter>
        )}
      </section>

      {launchCommand && (
        <section>
          <GroupTitle>{t("deck.launchCommand")}</GroupTitle>
          <Group>
            <div className="flex items-start gap-3 px-3.5 py-3">
              <code className="min-w-0 flex-1 break-all font-mono text-sm text-foreground">
                {launchCommand}
              </code>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label={t("common.copy")}
                onClick={() => {
                  void copyText(launchCommand);
                  toast.success(t("common.copied"));
                }}
              >
                <Copy />
              </Button>
            </div>
          </Group>
          <GroupFooter>
            {t(
              client === "dsh"
                ? "deck.dshLaunchHint"
                : client === "copilot"
                  ? "deck.copilotLaunchHint"
                  : "deck.launchCommandHint",
            )}
          </GroupFooter>
        </section>
      )}

      {active && (
        <section>
          <GroupTitle>{t("deck.appliedState")}</GroupTitle>
          <Group>
            <div className="flex items-center gap-2.5 px-3.5 py-2.5 text-base">
              <CircleCheck className="size-4 shrink-0 text-success" />
              <span className="min-w-0 flex-1 truncate">
                {view.store.endpoints[active.endpointId]?.name ??
                  active.endpointId}
              </span>
              <Badge variant="secondary">{active.model}</Badge>
            </div>
          </Group>
        </section>
      )}
    </Page>
  );
}
