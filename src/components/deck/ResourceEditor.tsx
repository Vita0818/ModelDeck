import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  PROTOCOLS,
  PROTOCOL_LABELS,
  type DeckStore,
  type ModelResource,
  type Protocol,
  type UpstreamProvider,
} from "@/lib/api/deck";
import { toast } from "sonner";

export const selectStyle =
  "h-9 w-full rounded-md border border-input bg-background px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";
export const newId = () => crypto.randomUUID();

export function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex min-w-0 flex-col gap-1.5 text-sm">
      <span className="text-muted-foreground">{label}</span>
      {children}
    </label>
  );
}

export default function ResourceEditor({
  store,
  onChange,
  disabled,
}: {
  store: DeckStore;
  onChange: (store: DeckStore) => void;
  disabled: boolean;
}) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState("");
  const [modelId, setModelId] = useState("");
  const [keySources, setKeySources] = useState<
    Record<string, "env" | "stored">
  >({});
  const providers = Object.values(store.providers);
  const provider = store.providers[selected] ?? providers[0];
  const models = Object.values(store.models).filter(
    (m) => m.providerId === provider?.id,
  );
  const model =
    store.models[modelId]?.providerId === provider?.id
      ? store.models[modelId]
      : models[0];
  const keySource = provider
    ? (keySources[provider.id] ??
      provider.keySource ??
      (provider.keyEnv ? "env" : "stored"))
    : "stored";
  const updateProvider = (patch: Partial<UpstreamProvider>) => {
    if (!provider) return;
    onChange({
      ...store,
      providers: {
        ...store.providers,
        [provider.id]: { ...provider, ...patch },
      },
    });
  };
  const updateModel = (patch: Partial<ModelResource>) => {
    if (!model) return;
    onChange({
      ...store,
      models: { ...store.models, [model.id]: { ...model, ...patch } },
    });
  };
  const addProvider = () => {
    const id = newId();
    onChange({
      ...store,
      providers: {
        ...store.providers,
        [id]: {
          id,
          name: t("deck.newProvider"),
          baseUrl: "",
          protocol: "openai-chat",
          apiKey: "",
          keyEnv: "",
        },
      },
    });
    setSelected(id);
    setModelId("");
  };
  const referenced = (ids: string[]) =>
    Object.values(store.endpoints).some((e) =>
      Object.values(e.routes).some((r) => ids.includes(r.modelId)),
    );
  const deleteProvider = () => {
    if (!provider) return;
    const ids = models.map((m) => m.id);
    if (referenced(ids)) {
      toast.error(t("deck.resourceInUse"));
      return;
    }
    const nextProviders = { ...store.providers };
    delete nextProviders[provider.id];
    const nextModels = { ...store.models };
    ids.forEach((id) => delete nextModels[id]);
    onChange({ ...store, providers: nextProviders, models: nextModels });
    setSelected("");
    setModelId("");
  };
  const addModel = () => {
    if (!provider) return;
    const id = newId();
    onChange({
      ...store,
      models: {
        ...store.models,
        [id]: {
          id,
          providerId: provider.id,
          upstreamId: "",
          name: t("deck.newModel"),
          context: 200000,
          output: 8192,
          reasoning: [],
          reasoningSupported: null,
          tools: true,
          images: false,
        },
      },
    });
    setModelId(id);
  };
  const deleteModel = () => {
    if (!model) return;
    if (referenced([model.id])) {
      toast.error(t("deck.resourceInUse"));
      return;
    }
    const next = { ...store.models };
    delete next[model.id];
    onChange({ ...store, models: next });
    setModelId("");
  };
  return (
    <div className="grid gap-6 lg:grid-cols-[230px_minmax(0,1fr)]">
      <aside className="min-w-0 space-y-2 border-b pb-4 lg:border-b-0 lg:border-r lg:pr-5">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="font-medium">{t("deck.providers")}</h3>
          <Button
            variant="ghost"
            size="icon"
            onClick={addProvider}
            disabled={disabled}
            aria-label={t("deck.addProvider")}
          >
            <Plus className="h-4 w-4" />
          </Button>
        </div>
        {providers.map((p) => (
          <button
            key={p.id}
            type="button"
            onClick={() => {
              setSelected(p.id);
              setModelId("");
            }}
            disabled={disabled}
            className={`w-full rounded-md px-3 py-2.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${provider?.id === p.id ? "bg-primary/10 text-primary" : "hover:bg-muted"}`}
          >
            <span className="block truncate text-sm font-medium">{p.name}</span>
            <span className="block text-xs text-muted-foreground">
              {
                Object.values(store.models).filter((m) => m.providerId === p.id)
                  .length
              }{" "}
              {t("deck.modelsCount")}
            </span>
          </button>
        ))}
        {!providers.length && (
          <p className="text-sm text-muted-foreground">
            {t("deck.addFirstProvider")}
          </p>
        )}
      </aside>
      {provider && (
        <fieldset disabled={disabled} className="min-w-0 space-y-6">
          <div className="flex items-center justify-between gap-3">
            <h3 className="text-base font-semibold">{provider.name}</h3>
            <Button variant="ghost" size="sm" onClick={deleteProvider}>
              <Trash2 className="mr-2 h-4 w-4" />
              {t("deck.deleteProvider")}
            </Button>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label={t("deck.providerName")}>
              <Input
                value={provider.name}
                onChange={(e) => updateProvider({ name: e.target.value })}
              />
            </Field>
            <Field label={t("deck.upstreamProtocol")}>
              <select
                className={selectStyle}
                value={provider.protocol}
                onChange={(e) =>
                  updateProvider({ protocol: e.target.value as Protocol })
                }
              >
                {PROTOCOLS.map((p) => (
                  <option key={p} value={p}>
                    {PROTOCOL_LABELS[p]}
                  </option>
                ))}
              </select>
            </Field>
            <Field label={t("deck.upstreamUrl")}>
              <Input
                value={provider.baseUrl}
                placeholder="https://api.example.com/v1"
                onChange={(e) => updateProvider({ baseUrl: e.target.value })}
              />
            </Field>
            <Field label={t("deck.credentialSource")}>
              <select
                className={selectStyle}
                value={keySource}
                onChange={(e) => {
                  const source = e.target.value as "env" | "stored";
                  setKeySources((previous) => ({
                    ...previous,
                    [provider.id]: source,
                  }));
                  updateProvider({
                    keySource: source,
                    ...(source === "env"
                      ? {
                          apiKey: "",
                          keyEnv: provider.keyEnv || "MODELDECK_API_KEY",
                        }
                      : { keyEnv: "" }),
                  });
                }}
              >
                <option value="stored">{t("deck.storedKey")}</option>
                <option value="env">{t("deck.environmentKey")}</option>
              </select>
            </Field>
            {keySource === "env" ? (
              <Field label={t("deck.keyVariable")}>
                <Input
                  value={provider.keyEnv}
                  onChange={(e) => updateProvider({ keyEnv: e.target.value })}
                />
              </Field>
            ) : (
              <Field label={t("deck.apiKey")}>
                <Input
                  type="password"
                  autoComplete="new-password"
                  value={provider.apiKey}
                  onChange={(e) => updateProvider({ apiKey: e.target.value })}
                />
              </Field>
            )}
          </div>
          <section className="border-t pt-5">
            <div className="mb-3 flex items-center justify-between">
              <h3 className="font-medium">{t("deck.modelResources")}</h3>
              <Button variant="outline" size="sm" onClick={addModel}>
                <Plus className="mr-2 h-4 w-4" />
                {t("deck.addModel")}
              </Button>
            </div>
            <div className="max-h-56 overflow-auto rounded-md border">
              <table className="w-full text-left text-sm">
                <thead className="sticky top-0 bg-muted">
                  <tr>
                    <th className="px-3 py-2 font-medium">
                      {t("deck.modelName")}
                    </th>
                    <th className="px-3 py-2 font-medium">
                      {t("deck.upstreamModel")}
                    </th>
                    <th className="px-3 py-2 text-right font-medium">
                      {t("deck.context")}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {models.map((m) => (
                    <tr
                      key={m.id}
                      className={
                        model?.id === m.id
                          ? "bg-primary/10"
                          : "hover:bg-muted/60"
                      }
                    >
                      <td className="px-3 py-2">
                        <button
                          type="button"
                          className="text-left font-medium underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                          onClick={() => setModelId(m.id)}
                        >
                          {m.name}
                        </button>
                      </td>
                      <td className="px-3 py-2 font-mono text-xs">
                        {m.upstreamId || "—"}
                      </td>
                      <td className="px-3 py-2 text-right font-mono text-xs">
                        {m.context.toLocaleString()}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {!models.length && (
                <p className="px-3 py-6 text-sm text-muted-foreground">
                  {t("deck.addFirstModel")}
                </p>
              )}
            </div>
            {model && (
              <div className="mt-5 space-y-4">
                <div className="flex items-center justify-between">
                  <h4 className="text-sm font-medium">{t("deck.editModel")}</h4>
                  <Button variant="ghost" size="sm" onClick={deleteModel}>
                    <Trash2 className="mr-2 h-4 w-4" />
                    {t("deck.deleteModel")}
                  </Button>
                </div>
                <div className="grid gap-4 sm:grid-cols-2">
                  <Field label={t("deck.modelName")}>
                    <Input
                      value={model.name}
                      onChange={(e) => updateModel({ name: e.target.value })}
                    />
                  </Field>
                  <Field label={t("deck.upstreamModel")}>
                    <Input
                      value={model.upstreamId}
                      onChange={(e) =>
                        updateModel({ upstreamId: e.target.value })
                      }
                    />
                  </Field>
                  <Field label={t("deck.context")}>
                    <Input
                      type="number"
                      min={1}
                      value={model.context}
                      onChange={(e) =>
                        updateModel({ context: Number(e.target.value) })
                      }
                    />
                  </Field>
                  <Field label={t("deck.output")}>
                    <Input
                      type="number"
                      min={1}
                      value={model.output}
                      onChange={(e) =>
                        updateModel({ output: Number(e.target.value) })
                      }
                    />
                  </Field>
                  <div className="space-y-2">
                    <p className="text-sm text-muted-foreground">
                      {t("deck.reasoningLevels")}
                    </p>
                    <div className="flex flex-wrap gap-x-4 gap-y-2">
                      {[
                        "none",
                        "minimal",
                        "low",
                        "medium",
                        "high",
                        "xhigh",
                        "max",
                        "ultra",
                      ].map((level) => (
                        <label
                          key={level}
                          className="flex items-center gap-1.5 text-sm"
                        >
                          <input
                            type="checkbox"
                            checked={model.reasoning.includes(level)}
                            onChange={(e) =>
                              updateModel({
                                reasoning: e.target.checked
                                  ? [...model.reasoning, level]
                                  : model.reasoning.filter((v) => v !== level),
                                reasoningSupported: true,
                              })
                            }
                          />
                          {level}
                        </label>
                      ))}
                    </div>
                  </div>
                  <div className="flex flex-wrap items-end gap-5 pb-2 text-sm">
                    <label className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={model.tools}
                        onChange={(e) =>
                          updateModel({ tools: e.target.checked })
                        }
                      />
                      {t("deck.tools")}
                    </label>
                    <label className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={model.images}
                        onChange={(e) =>
                          updateModel({ images: e.target.checked })
                        }
                      />
                      {t("deck.images")}
                    </label>
                  </div>
                </div>
              </div>
            )}
          </section>
        </fieldset>
      )}
    </div>
  );
}
