import { Monitor, Moon, Sun } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Segmented } from "@/components/ui/segmented";
import { useTheme } from "@/components/theme-provider";

/** Row content only; the enclosing Group supplies the card and separators. */
export function ThemeSettings() {
  const { t } = useTranslation();
  const { theme, setTheme } = useTheme();

  return (
    <Segmented
      aria-label={t("settings.theme")}
      value={theme}
      onChange={setTheme}
      options={[
        { value: "light", label: t("settings.themeLight"), icon: Sun },
        { value: "dark", label: t("settings.themeDark"), icon: Moon },
        { value: "system", label: t("settings.themeSystem"), icon: Monitor },
      ]}
    />
  );
}
