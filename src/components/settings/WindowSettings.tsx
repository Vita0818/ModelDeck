import { useTranslation } from "react-i18next";
import type { SettingsFormState } from "@/hooks/useSettings";
import { AppWindow, EyeOff, Minimize2, Power } from "lucide-react";
import { ToggleRow } from "@/components/ui/toggle-row";
import { isLinux } from "@/lib/platform";

interface WindowSettingsProps {
  settings: SettingsFormState;
  onChange: (updates: Partial<SettingsFormState>) => void;
}

export function WindowSettings({ settings, onChange }: WindowSettingsProps) {
  const { t } = useTranslation();

  return (
    <>
      <ToggleRow
        icon={<Power className="size-4" />}
        title={t("settings.launchOnStartup")}
        description={t("settings.launchOnStartupDescription")}
        checked={!!settings.launchOnStartup}
        onCheckedChange={(value) => onChange({ launchOnStartup: value })}
      />
      {settings.launchOnStartup && (
        <ToggleRow
          icon={<EyeOff className="size-4" />}
          title={t("settings.silentStartup")}
          description={t("settings.silentStartupDescription")}
          checked={!!settings.silentStartup}
          onCheckedChange={(value) => onChange({ silentStartup: value })}
        />
      )}
      <ToggleRow
        icon={<Minimize2 className="size-4" />}
        title={t("settings.minimizeToTray")}
        description={t("settings.minimizeToTrayDescription")}
        checked={!!settings.minimizeToTrayOnClose}
        onCheckedChange={(value) => onChange({ minimizeToTrayOnClose: value })}
      />
      {isLinux() && (
        <ToggleRow
          icon={<AppWindow className="size-4" />}
          title={t("settings.useAppWindowControls")}
          description={t("settings.useAppWindowControlsDescription")}
          checked={!!settings.useAppWindowControls}
          onCheckedChange={(value) => onChange({ useAppWindowControls: value })}
        />
      )}
    </>
  );
}
