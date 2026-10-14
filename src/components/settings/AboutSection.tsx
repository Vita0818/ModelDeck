import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Group, Row } from "@/components/ui/group";
import appIcon from "@/assets/icons/app-icon.png";
import { APP_NAME, UPSTREAM_URL } from "@/lib/product";

export function AboutSection({ isPortable }: { isPortable: boolean }) {
  const { t } = useTranslation();
  const [version, setVersion] = useState("—");
  useEffect(() => {
    void getVersion().then(setVersion).catch(console.error);
  }, []);
  const open = (url: string) => {
    void invoke("open_external", { url });
  };
  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <img src={appIcon} alt="" className="size-16 rounded-2xl" />
        <div>
          <h2 className="text-xl font-semibold">{APP_NAME}</h2>
          <p className="text-sm text-muted-foreground">v{version}</p>
        </div>
      </div>
      <p className="text-sm text-muted-foreground">
        {t("settings.aboutIndependentDescription")}
      </p>
      <Group>
        <Row
          label={t("settings.aboutProject")}
          description="Vita0818/ModelDeck"
          trailing={
            <Button
              variant="ghost"
              size="sm"
              onClick={() => open("https://github.com/Vita0818/ModelDeck")}
              aria-label={t("settings.aboutProject")}
            >
              <ExternalLink className="size-4" />
            </Button>
          }
        />
        <Row
          label={t("settings.aboutBuild")}
          description={t(
            isPortable
              ? "settings.aboutPortableBuild"
              : "settings.aboutLocalBuild",
          )}
        />
        <Row
          label={t("settings.aboutSource")}
          description={t("settings.aboutSourceNotice")}
          trailing={
            <Button
              variant="ghost"
              size="sm"
              onClick={() => open(UPSTREAM_URL)}
              aria-label={t("settings.aboutSource")}
            >
              <ExternalLink className="size-4" />
            </Button>
          }
        />
      </Group>
    </div>
  );
}
