import { useTranslation } from "react-i18next";
import { Segmented } from "@/components/ui/segmented";

type LanguageOption = "zh" | "zh-TW" | "en" | "ja";

interface LanguageSettingsProps {
  value: LanguageOption;
  onChange: (value: LanguageOption) => void;
}

/** Row content only; the enclosing Group supplies the card and separators. */
export function LanguageSettings({ value, onChange }: LanguageSettingsProps) {
  const { t } = useTranslation();

  return (
    <Segmented<LanguageOption>
      aria-label={t("settings.language")}
      value={value}
      onChange={onChange}
      options={[
        { value: "zh", label: t("settings.languageOptionChinese") },
        {
          value: "zh-TW",
          label: t("settings.languageOptionTraditionalChinese"),
        },
        { value: "en", label: t("settings.languageOptionEnglish") },
        { value: "ja", label: t("settings.languageOptionJapanese") },
      ]}
    />
  );
}
