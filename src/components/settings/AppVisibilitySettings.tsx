import { useTranslation } from "react-i18next";
import { ProviderIcon } from "@/components/ProviderIcon";
import { Group, GroupFooter, GroupTitle } from "@/components/ui/group";
import { ToggleRow } from "@/components/ui/toggle-row";
import type { SettingsFormState } from "@/hooks/useSettings";
import type { VisibleApps } from "@/types";
import {
  MODELDECK_CLIENTS,
  resolveClientVisibility,
  type ClientId,
} from "@/config/modeldeckClients";

interface AppVisibilitySettingsProps {
  settings: SettingsFormState;
  onChange: (updates: Partial<SettingsFormState>) => void;
}

export function AppVisibilitySettings({
  settings,
  onChange,
}: AppVisibilitySettingsProps) {
  const { t } = useTranslation();

  const visibleApps: VisibleApps = resolveClientVisibility(
    settings.visibleApps,
  );

  // Count how many apps are currently visible
  const visibleCount = MODELDECK_CLIENTS.filter(
    ({ id }) => visibleApps[id],
  ).length;

  const handleToggle = (appId: ClientId) => {
    const isCurrentlyVisible = visibleApps[appId];
    // Prevent disabling the last visible app
    if (isCurrentlyVisible && visibleCount <= 1) return;

    onChange({
      visibleApps: {
        ...visibleApps,
        [appId]: !isCurrentlyVisible,
      },
    });
  };

  return (
    <section>
      <GroupTitle>{t("settings.appVisibility.title")}</GroupTitle>
      <Group>
        {MODELDECK_CLIENTS.map((app) => {
          const isVisible = visibleApps[app.id] ?? true;
          return (
            <ToggleRow
              key={app.id}
              icon={<ProviderIcon icon={app.icon} name={app.name} size={18} />}
              title={app.name}
              checked={isVisible}
              // Hiding the last visible client would leave no destination.
              disabled={isVisible && visibleCount <= 1}
              onCheckedChange={() => handleToggle(app.id)}
            />
          );
        })}
      </Group>
      <GroupFooter>{t("settings.appVisibility.description")}</GroupFooter>
    </section>
  );
}
