import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Activity, Loader2, Play, Save, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { proxyApi } from "@/lib/api/proxy";
import { useDeck } from "@/lib/query/deck";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Group, GroupTitle, Row } from "@/components/ui/group";
import { GlobalProxySettings } from "./GlobalProxySettings";

export function ProxyTabContent() {
  const { t } = useTranslation();
  const cache = useQueryClient();
  const config = useQuery({
    queryKey: ["gateway-config"],
    queryFn: () => proxyApi.getGlobalProxyConfig(),
  });
  const status = useQuery({
    queryKey: ["gateway-status"],
    queryFn: () => proxyApi.getProxyStatus(),
    refetchInterval: 5000,
  });
  const { data: deck } = useDeck();
  const activeClients = Object.keys(deck?.store.activeBindings ?? {}).length;
  const [address, setAddress] = useState("127.0.0.1");
  const [port, setPort] = useState("15721");
  useEffect(() => {
    if (config.data) {
      setAddress(config.data.listenAddress);
      setPort(String(config.data.listenPort));
    }
  }, [config.data]);
  const refresh = () =>
    Promise.all([
      cache.invalidateQueries({ queryKey: ["gateway-config"] }),
      cache.invalidateQueries({ queryKey: ["gateway-status"] }),
    ]);
  const reportError = (error: unknown) => toast.error(String(error));
  const save = useMutation({
    mutationFn: async () => {
      if (!config.data) throw new Error(t("settings.gateway.unavailable"));
      await proxyApi.updateGlobalProxyConfig({
        ...config.data,
        listenAddress: address.trim(),
        listenPort: Number(port),
      });
    },
    onSuccess: async () => {
      await refresh();
      toast.success(t("settings.gateway.saved"));
    },
    onError: reportError,
  });
  const control = useMutation({
    mutationFn: async () =>
      status.data?.running
        ? proxyApi.stopProxyServer()
        : proxyApi.startProxyServer(),
    onSuccess: refresh,
    onError: reportError,
  });
  const busy = save.isPending || control.isPending;
  const running = status.data?.running ?? false;
  const changed =
    Boolean(config.data) &&
    (address.trim() !== config.data?.listenAddress ||
      port !== String(config.data?.listenPort));

  return (
    <div className="space-y-7">
      <section>
        <GroupTitle>{t("settings.gateway.title")}</GroupTitle>
        <Group>
          <Row
            label={t("settings.gateway.status")}
            description={
              running
                ? `${status.data?.address}:${status.data?.port}`
                : t("settings.gateway.stopped")
            }
            trailing={
              <Button
                variant="outline"
                size="sm"
                disabled={
                  busy || !status.data || (running && activeClients > 0)
                }
                onClick={() => control.mutate()}
              >
                {control.isPending ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : running ? (
                  <Square className="size-4" />
                ) : (
                  <Play className="size-4" />
                )}
                {t(
                  running ? "settings.gateway.stop" : "settings.gateway.start",
                )}
              </Button>
            }
          />
        </Group>
        <form
          className="mt-4 space-y-4"
          onSubmit={(event) => {
            event.preventDefault();
            save.mutate();
          }}
        >
          <div className="grid gap-4 sm:grid-cols-[1fr_10rem]">
            <div className="space-y-2">
              <Label htmlFor="gateway-address">
                {t("settings.gateway.address")}
              </Label>
              <Input
                id="gateway-address"
                value={address}
                required
                disabled={busy || activeClients > 0 || !config.data}
                onChange={(event) => setAddress(event.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="gateway-port">{t("settings.gateway.port")}</Label>
              <Input
                id="gateway-port"
                type="number"
                min={1}
                max={65535}
                step={1}
                value={port}
                required
                disabled={busy || activeClients > 0 || !config.data}
                onChange={(event) => setPort(event.target.value)}
              />
            </div>
          </div>
          <p className="text-xs text-muted-foreground">
            {t("settings.gateway.hint")}
          </p>
          {(config.error || status.error) && (
            <p role="alert" className="text-sm text-destructive">
              {String(config.error || status.error)}
            </p>
          )}
          <Button
            type="submit"
            disabled={busy || !changed || activeClients > 0}
          >
            {save.isPending ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Save className="size-4" />
            )}
            {t("common.save")}
          </Button>
        </form>
      </section>
      <section>
        <GroupTitle>
          <Activity className="mr-2 inline size-4" />
          {t("settings.gateway.upstream")}
        </GroupTitle>
        <GlobalProxySettings />
      </section>
    </div>
  );
}
