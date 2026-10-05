import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Check, Copy, Loader2, Plus, Save, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { ModelClientId } from "@/config/modeldeckClients";

type Client = ModelClientId;
interface Model {
  id: string;
  name: string;
  context: number;
  output: number;
  reasoning: string[];
}
interface Profile {
  connectionName?: string;
  endpoint: string;
  protocol: string;
  apiKey: string;
  keyEnv: string;
  models: Model[];
  defaultModel: string;
  utilityModel?: string;
  effort: string;
  executable: string;
  enabled: boolean;
  localToken: string;
}
interface Inspection {
  client: Client;
  installed: boolean;
  executable?: string;
  configuration: string;
  capability: string;
  version?: string;
}
interface Source {
  id: string;
  name: string;
  count: number;
}
interface Store {
  profiles: Partial<Record<Client, Profile>>;
  active: Partial<Record<Client, Profile>>;
  savedProfiles?: Partial<Record<Client, Record<string, Profile>>>;
}
const clients: { id: Client; name: string; key: string }[] = [
  { id: "antigravity", name: "Antigravity CLI", key: "AGY_OR_KEY" },
  { id: "cursor", name: "Cursor IDE", key: "CURSOR_OR_KEY" },
  { id: "cursor-agent", name: "Cursor Agent CLI", key: "CURSOR_CLI_OR_KEY" },
  { id: "muse", name: "Muse Code", key: "MUSE_OR_KEY" },
];
const blank = (client: Client, executable = ""): Profile => ({
  connectionName: "OpenRouter",
  endpoint: "https://openrouter.ai/api/v1",
  protocol: client === "muse" ? "openai-responses" : "openai-chat",
  apiKey: "",
  keyEnv: clients.find((c) => c.id === client)!.key,
  models: [],
  defaultModel: "",
  utilityModel: "",
  effort: "",
  executable,
  enabled: false,
  localToken: "",
});
const selectStyle =
  "h-9 w-full rounded-md border border-border bg-background px-3 text-sm";

export default function ModelClientsPanel({
  selectedClient,
  onBusyChange,
  visible = true,
}: {
  selectedClient?: Client;
  onBusyChange?: (busy: boolean) => void;
  visible?: boolean;
}) {
  const { t } = useTranslation();
  const [localClient, setClient] = useState<Client>("antigravity");
  const client = selectedClient ?? localClient;
  const [profiles, setProfiles] = useState<Partial<Record<Client, Profile>>>(
    {},
  );
  const [inspection, setInspection] = useState<Inspection[]>([]);
  const [sources, setSources] = useState<Source[]>([]);
  const [sourceId, setSourceId] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [command, setCommand] = useState("");
  const [keySources, setKeySources] = useState<
    Partial<Record<Client, "env" | "stored">>
  >({});
  const [active, setActive] = useState<Store["active"]>({});
  const [savedProfiles, setSavedProfiles] = useState<
    NonNullable<Store["savedProfiles"]>
  >({});
  const wasVisible = useRef(visible);
  useEffect(() => {
    const returning = visible && !wasVisible.current;
    wasVisible.current = visible;
    if (!returning) return;
    let cancelled = false;
    // Providers can be edited on another client page while this draft stays
    // mounted. Refresh catalog metadata without reloading its credentials.
    Promise.all([
      invoke<Source[]>("get_model_client_catalog_sources"),
      invoke<Inspection[]>("inspect_model_clients"),
    ])
      .then(([catalogs, installed]) => {
        if (cancelled) return;
        setSources(catalogs);
        setInspection(installed);
        setSourceId((current) =>
          catalogs.some((source) => source.id === current)
            ? current
            : (catalogs.find((source) => /openrouter/i.test(source.name))?.id ??
              catalogs[0]?.id ??
              ""),
        );
      })
      .catch((error) => {
        if (!cancelled) setError(String(error));
      });
    return () => {
      cancelled = true;
    };
  }, [visible]);
  useEffect(() => {
    onBusyChange?.(busy);
  }, [busy, onBusyChange]);
  useEffect(() => {
    setError("");
    setCommand("");
  }, [client]);
  useEffect(() => {
    let cancelled = false;
    Promise.all([
      invoke<Store>("get_model_clients"),
      invoke<Inspection[]>("inspect_model_clients"),
      invoke<Source[]>("get_model_client_catalog_sources"),
    ])
      .then(([store, installed, catalogs]) => {
        if (cancelled) return;
        setProfiles(store.profiles);
        setActive(store.active);
        setSavedProfiles(store.savedProfiles ?? {});
        setInspection(installed);
        setSources(catalogs);
        setSourceId(
          catalogs.find((source) => /openrouter/i.test(source.name))?.id ??
            catalogs[0]?.id ??
            "",
        );
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);
  const status = inspection.find((i) => i.client === client);
  const profile = profiles[client] ?? blank(client, status?.executable);
  const keySource = keySources[client] ?? (profile.keyEnv ? "env" : "stored");
  const model = profile.models.find((m) => m.id === profile.defaultModel);
  const cursorCloud = client === "cursor" || client === "cursor-agent";
  const update = (patch: Partial<Profile>) =>
    setProfiles((previous) => ({
      ...previous,
      [client]: { ...profile, ...patch },
    }));
  const updateModel = (index: number, patch: Partial<Model>) => {
    const models = profile.models.map((m, i) =>
      i === index ? { ...m, ...patch } : m,
    );
    update({
      models,
      ...(patch.id !== undefined &&
      profile.models[index].id === profile.defaultModel
        ? { defaultModel: patch.id }
        : {}),
      ...(patch.id !== undefined &&
      profile.models[index].id === profile.utilityModel
        ? { utilityModel: patch.id }
        : {}),
    });
  };
  const chooseDefault = (id: string) => {
    const levels = profile.models.find((m) => m.id === id)?.reasoning ?? [];
    update({
      defaultModel: id,
      effort: cursorCloud
        ? ""
        : (["max", "xhigh", "high"].find((e) => levels.includes(e)) ?? ""),
    });
  };
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const save = async (apply: boolean) =>
    run(async () => {
      await invoke("save_model_client", { client, profile });
      if (profile.connectionName?.trim()) {
        const name = profile.connectionName.trim();
        setSavedProfiles((previous) => ({
          ...previous,
          [client]: {
            ...previous[client],
            [name]: { ...profile, connectionName: name },
          },
        }));
      }
      if (apply) {
        const result = await invoke<{
          command?: string;
          restartRequired: boolean;
        }>("apply_model_client", { client });
        setCommand(result.command ?? "");
        setActive((previous) => ({ ...previous, [client]: profile }));
        toast.success(
          t(
            result.restartRequired
              ? "modelClients.restartCursor"
              : "modelClients.applied",
          ),
        );
      } else toast.success(t("modelClients.saved"));
    });
  const importCatalog = () =>
    run(async () => {
      const models = await invoke<Model[]>("import_model_client_catalog", {
        providerId: sourceId,
      });
      if (!models.length) throw new Error(t("modelClients.emptyCatalog"));
      const combined = [
        ...profile.models,
        ...models.filter((m) => !profile.models.some((old) => old.id === m.id)),
      ];
      const chosen =
        combined.find((m) => m.id === profile.defaultModel) ??
        combined.find((m) => m.id === "openai/gpt-6-astra") ??
        combined[0];
      update({
        models: combined,
        defaultModel: chosen.id,
        effort: cursorCloud
          ? ""
          : (["max", "xhigh", "high"].find((e) =>
              chosen.reasoning.includes(e),
            ) ?? ""),
      });
    });
  const reset = () =>
    run(async () => {
      await invoke("reset_model_client", { client });
      setActive((previous) => {
        const next = { ...previous };
        delete next[client];
        return next;
      });
      setCommand("");
      toast.success(t("modelClients.resetDone"));
    });
  if (loading)
    return (
      <div className="p-8" role="status">
        <Loader2
          className="animate-spin"
          aria-label={t("modelClients.loading")}
        />
      </div>
    );
  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-6 pb-8">
      <div className="mx-auto max-w-6xl space-y-5">
        <p className="text-sm text-muted-foreground">
          {t("modelClients.description")}
        </p>
        {!selectedClient && (
          <div
            className="flex flex-wrap gap-2"
            role="tablist"
            aria-label={t("modelClients.title")}
          >
            {clients.map((c) => (
              <Button
                key={c.id}
                role="tab"
                aria-selected={client === c.id}
                variant={client === c.id ? "default" : "outline"}
                disabled={busy}
                onClick={() => {
                  setClient(c.id);
                  setError("");
                  setCommand("");
                }}
              >
                {c.name}
                {active[c.id] && <Check className="ml-2 h-3 w-3" />}
              </Button>
            ))}
          </div>
        )}
        <div className="rounded-xl border border-border bg-muted/40 p-4 text-sm space-y-2">
          <p>{t(`modelClients.capability.${client}`)}</p>
          <p className="text-muted-foreground break-all">
            {t(
              status?.installed
                ? "modelClients.detected"
                : "modelClients.notDetected",
            )}
            {status?.version && ` · ${status.version}`}
            {status?.configuration && ` · ${status.configuration}`}
          </p>
        </div>
        {error && (
          <div
            role="alert"
            className="rounded-lg border border-destructive/40 bg-destructive/5 p-4 text-sm text-destructive whitespace-pre-wrap break-words"
          >
            {error}
          </div>
        )}
        <fieldset disabled={busy} className="space-y-5">
          <div className="grid gap-4 md:grid-cols-2">
            <label className="space-y-2 text-sm">
              <span>{t("modelClients.connectionName")}</span>
              <Input
                value={profile.connectionName ?? ""}
                maxLength={80}
                onChange={(e) => update({ connectionName: e.target.value })}
              />
            </label>
            <label className="space-y-2 text-sm">
              <span>{t("modelClients.savedConnections")}</span>
              <select
                className={selectStyle}
                value={
                  savedProfiles[client]?.[profile.connectionName ?? ""]
                    ? profile.connectionName
                    : ""
                }
                onChange={(e) => {
                  const selected = savedProfiles[client]?.[e.target.value];
                  if (selected) {
                    setKeySources((previous) => ({
                      ...previous,
                      [client]: selected.keyEnv ? "env" : "stored",
                    }));
                    setProfiles((previous) => ({
                      ...previous,
                      [client]: { ...selected },
                    }));
                    setError("");
                    setCommand("");
                  }
                }}
              >
                <option value="">{t("modelClients.chooseConnection")}</option>
                {Object.keys(savedProfiles[client] ?? {}).map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
            {active[client] && (
              <p className="text-xs text-muted-foreground md:col-span-2">
                {t("modelClients.activeConnection")}{" "}
                {active[client]?.connectionName || active[client]?.endpoint}
              </p>
            )}
          </div>
          <div className="grid gap-4 rounded-xl border border-border p-5 md:grid-cols-2">
            <label className="space-y-2 text-sm">
              <span>{t("modelClients.endpoint")}</span>
              <Input
                value={profile.endpoint}
                placeholder="https://openrouter.ai/api/v1"
                onChange={(e) => update({ endpoint: e.target.value })}
              />
            </label>
            <label className="space-y-2 text-sm">
              <span>{t("modelClients.protocol")}</span>
              <select
                className={selectStyle}
                value={profile.protocol}
                onChange={(e) => update({ protocol: e.target.value })}
              >
                <option value="openai-chat">OpenAI Chat Completions</option>
                {client === "muse" && (
                  <>
                    <option value="openai-responses">OpenAI Responses</option>
                    <option value="anthropic">Anthropic Messages</option>
                  </>
                )}
                {client === "antigravity" && (
                  <option value="gemini">Gemini</option>
                )}
              </select>
            </label>
            <label className="space-y-2 text-sm">
              <span>{t("modelClients.keySource")}</span>
              <select
                className={selectStyle}
                value={keySource}
                onChange={(e) => {
                  const source = e.target.value === "env" ? "env" : "stored";
                  setKeySources((previous) => ({
                    ...previous,
                    [client]: source,
                  }));
                  update(
                    source === "env"
                      ? {
                          keyEnv: clients.find((c) => c.id === client)!.key,
                          apiKey: "",
                        }
                      : { keyEnv: "", apiKey: "" },
                  );
                }}
              >
                <option value="env">{t("modelClients.environment")}</option>
                <option value="stored">{t("modelClients.storedKey")}</option>
              </select>
            </label>
            {keySource === "env" ? (
              <label className="space-y-2 text-sm">
                <span>{t("modelClients.envName")}</span>
                <Input
                  value={profile.keyEnv}
                  onChange={(e) => {
                    setKeySources((previous) => ({
                      ...previous,
                      [client]: "env",
                    }));
                    update({ keyEnv: e.target.value });
                  }}
                />
              </label>
            ) : (
              <label className="space-y-2 text-sm">
                <span>{t("modelClients.apiKey")}</span>
                <Input
                  type="password"
                  value={profile.apiKey}
                  onChange={(e) => update({ apiKey: e.target.value })}
                />
              </label>
            )}
            {client !== "cursor" && (
              <label className="space-y-2 text-sm md:col-span-2">
                <span>{t("modelClients.executable")}</span>
                <Input
                  value={profile.executable}
                  onChange={(e) => update({ executable: e.target.value })}
                />
              </label>
            )}
            <p className="text-xs text-muted-foreground md:col-span-2">
              {t("modelClients.keyHint")}
            </p>
          </div>
          <div className="rounded-xl border border-border p-5 space-y-4">
            <div className="flex flex-wrap gap-3 items-center justify-between">
              <h2 className="font-medium">
                {t("modelClients.catalog")}{" "}
                <span className="text-muted-foreground">
                  {profile.models.length}
                </span>
              </h2>
              <div className="flex flex-wrap gap-2">
                <select
                  aria-label={t("modelClients.catalogSource")}
                  className={`${selectStyle} w-auto max-w-60`}
                  value={sourceId}
                  onChange={(e) => setSourceId(e.target.value)}
                >
                  <option value="">{t("modelClients.catalogSource")}</option>
                  {sources.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} ({s.count})
                    </option>
                  ))}
                </select>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={!sourceId}
                  onClick={() => void importCatalog()}
                >
                  {t("modelClients.importModels")}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    update({
                      models: [
                        ...profile.models,
                        {
                          id: "",
                          name: "",
                          context: 1000000,
                          output: 32768,
                          reasoning: [],
                        },
                      ],
                    })
                  }
                >
                  <Plus className="mr-1 h-4 w-4" />
                  {t("modelClients.addModel")}
                </Button>
              </div>
            </div>
            <p className="text-xs text-muted-foreground">
              {t("modelClients.catalogHint")}
            </p>
            {cursorCloud && (
              <p className="text-xs text-muted-foreground">
                {t("modelClients.cursorLimits")}
              </p>
            )}
            {!profile.models.length ? (
              <p className="rounded-md bg-muted p-5 text-sm text-muted-foreground">
                {t("modelClients.empty")}
              </p>
            ) : (
              <div className="max-h-80 overflow-auto">
                <table className="w-full min-w-[800px] text-sm">
                  <thead>
                    <tr className="text-left text-muted-foreground">
                      {[
                        "modelId",
                        "displayName",
                        "context",
                        "output",
                        "levels",
                      ].map((k) => (
                        <th key={k} className="px-1 pb-2 font-normal">
                          {t(`modelClients.${k}`)}
                        </th>
                      ))}
                      <th>
                        <span className="sr-only">
                          {t("modelClients.remove")}
                        </span>
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {profile.models.map((m, i) => (
                      <tr key={i}>
                        <td className="p-1 min-w-56">
                          <Input
                            aria-label={`${t("modelClients.modelId")} ${i + 1}`}
                            value={m.id}
                            onChange={(e) =>
                              updateModel(i, { id: e.target.value })
                            }
                          />
                        </td>
                        <td className="p-1 min-w-44">
                          <Input
                            aria-label={`${t("modelClients.displayName")} ${i + 1}`}
                            value={m.name}
                            onChange={(e) =>
                              updateModel(i, { name: e.target.value })
                            }
                          />
                        </td>
                        <td className="p-1 w-32">
                          <Input
                            aria-label={`${t("modelClients.context")} ${m.id}`}
                            type="number"
                            min="1"
                            value={m.context}
                            onChange={(e) =>
                              updateModel(i, {
                                context: Number(e.target.value),
                              })
                            }
                          />
                        </td>
                        <td className="p-1 w-28">
                          <Input
                            aria-label={`${t("modelClients.output")} ${m.id}`}
                            type="number"
                            min="1"
                            value={m.output}
                            onChange={(e) =>
                              updateModel(i, { output: Number(e.target.value) })
                            }
                          />
                        </td>
                        <td className="p-1 min-w-40">
                          <Input
                            aria-label={`${t("modelClients.levels")} ${m.id}`}
                            value={m.reasoning.join(", ")}
                            placeholder="high, xhigh, max"
                            onChange={(e) =>
                              updateModel(i, {
                                reasoning: e.target.value
                                  .split(",")
                                  .map((s) => s.trim())
                                  .filter(Boolean),
                              })
                            }
                          />
                        </td>
                        <td className="p-1">
                          <Button
                            variant="ghost"
                            size="icon"
                            aria-label={`${t("modelClients.remove")} ${m.id}`}
                            onClick={() =>
                              update({
                                models: profile.models.filter(
                                  (_, j) => j !== i,
                                ),
                                ...(m.id === profile.defaultModel
                                  ? { defaultModel: "", effort: "" }
                                  : {}),
                                ...(m.id === profile.utilityModel
                                  ? { utilityModel: "" }
                                  : {}),
                              })
                            }
                          >
                            <Trash2 className="h-4 w-4" />
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            <div className="grid gap-4 md:grid-cols-2">
              <label className="space-y-2 text-sm">
                <span>{t("modelClients.defaultModel")}</span>
                <select
                  className={selectStyle}
                  value={profile.defaultModel}
                  onChange={(e) => chooseDefault(e.target.value)}
                >
                  <option value="">{t("modelClients.selectModel")}</option>
                  {profile.models
                    .filter((m) => m.id)
                    .map((m, i) => (
                      <option key={i} value={m.id}>
                        {m.name || m.id}
                      </option>
                    ))}
                </select>
              </label>
              <label className="space-y-2 text-sm">
                <span>{t("modelClients.effort")}</span>
                <select
                  className={selectStyle}
                  value={profile.effort}
                  disabled={!model?.reasoning.length || cursorCloud}
                  onChange={(e) => update({ effort: e.target.value })}
                >
                  <option value="">{t("modelClients.providerDefault")}</option>
                  {model?.reasoning.map((e) => (
                    <option key={e} value={e}>
                      {e}
                    </option>
                  ))}
                </select>
              </label>
              {client === "antigravity" && (
                <label className="space-y-2 text-sm md:col-span-2">
                  <span>{t("modelClients.utilityModel")}</span>
                  <select
                    className={selectStyle}
                    value={profile.utilityModel ?? ""}
                    onChange={(e) => update({ utilityModel: e.target.value })}
                  >
                    <option value="">{t("modelClients.sameAsDefault")}</option>
                    {profile.models
                      .filter((m) => m.id)
                      .map((m, i) => (
                        <option key={i} value={m.id}>
                          {m.name || m.id}
                        </option>
                      ))}
                  </select>
                  <p className="text-xs text-muted-foreground">
                    {t("modelClients.utilityHint")}
                  </p>
                </label>
              )}
            </div>
          </div>
        </fieldset>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-xs text-muted-foreground max-w-xl">
            {t(
              cursorCloud
                ? "modelClients.cursorApplyHint"
                : "modelClients.applyHint",
            )}
          </p>
          <div className="flex gap-2">
            {active[client] && (
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => void reset()}
              >
                {t("modelClients.reset")}
              </Button>
            )}
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => void save(false)}
            >
              <Save className="h-4 w-4 mr-2" />
              {t("modelClients.save")}
            </Button>
            <Button
              disabled={busy || !profile.defaultModel}
              onClick={() => void save(true)}
            >
              {busy && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {t("modelClients.apply")}
            </Button>
          </div>
        </div>
        {command && (
          <div className="rounded-xl border border-border p-4 space-y-2">
            <p className="text-sm font-medium">{t("modelClients.launch")}</p>
            <div className="flex gap-2 items-center">
              <code className="flex-1 text-sm break-all">{command}</code>
              <Button
                variant="ghost"
                size="icon"
                aria-label={t("modelClients.copy")}
                onClick={() => {
                  void navigator.clipboard
                    .writeText(command)
                    .then(() => toast.success(t("modelClients.copied")))
                    .catch((e) => setError(String(e)));
                }}
              >
                <Copy className="h-4 w-4" />
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              {t("modelClients.launchHint")}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
