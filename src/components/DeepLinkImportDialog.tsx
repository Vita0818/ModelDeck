import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { deeplinkApi, type DeepLinkImportRequest } from "@/lib/api/deeplink";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export function DeepLinkImportDialog() {
  const { t } = useTranslation();
  const cache = useQueryClient();
  const [request, setRequest] = useState<DeepLinkImportRequest | null>(null);
  const [open, setOpen] = useState(false);
  const [previewed, setPreviewed] = useState(false);
  useEffect(() => {
    const imported = listen<DeepLinkImportRequest>(
      "deeplink-import",
      ({ payload }) => {
        if (payload.resource !== "provider") return;
        setRequest(payload);
        setPreviewed(false);
        setOpen(true);
      },
    );
    const failed = listen<{ error: string }>("deeplink-error", ({ payload }) =>
      toast.error(payload.error),
    );
    return () => {
      void imported.then((fn) => fn());
      void failed.then((fn) => fn());
    };
  }, []);
  const reportError = (error: unknown) => toast.error(String(error));
  const preview = useMutation({
    mutationFn: () => deeplinkApi.mergeDeeplinkConfig(request!),
    onSuccess: (value) => {
      setRequest(value);
      setPreviewed(true);
    },
    onError: reportError,
  });
  const save = useMutation({
    mutationFn: () => deeplinkApi.importFromDeeplink(request!),
    onSuccess: async () => {
      await cache.invalidateQueries({ queryKey: ["deck"] });
      toast.success(t("deeplink.resourceImported"));
      setOpen(false);
    },
    onError: reportError,
  });
  const needsPreview =
    Boolean(request?.config || request?.configUrl) && !previewed;
  const busy = preview.isPending || save.isPending;
  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        if (!busy) setOpen(value);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t("deeplink.resourceTitle")}</DialogTitle>
          <DialogDescription>
            {t("deeplink.resourceDescription")}
          </DialogDescription>
        </DialogHeader>
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-3 text-sm">
          <dt className="text-muted-foreground">{t("common.name")}</dt>
          <dd className="break-all">{request?.name || "—"}</dd>
          <dt className="text-muted-foreground">
            {t("deeplink.resourceEndpoint")}
          </dt>
          <dd className="break-all">{request?.endpoint || "—"}</dd>
          <dt className="text-muted-foreground">
            {t("deeplink.resourceModel")}
          </dt>
          <dd className="break-all">{request?.model || "—"}</dd>
          <dt className="text-muted-foreground">API Key</dt>
          <dd>{request?.apiKey ? "••••••••" : "—"}</dd>
        </dl>
        <DialogFooter>
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => setOpen(false)}
          >
            {t("common.cancel")}
          </Button>
          {needsPreview ? (
            <Button disabled={busy} onClick={() => preview.mutate()}>
              {preview.isPending && <Loader2 className="size-4 animate-spin" />}
              {t("deeplink.resourcePreview")}
            </Button>
          ) : (
            <Button
              disabled={
                busy || !request?.name || !request.endpoint || !request.apiKey
              }
              onClick={() => save.mutate()}
            >
              {save.isPending && <Loader2 className="size-4 animate-spin" />}
              {t("deeplink.resourceImport")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
