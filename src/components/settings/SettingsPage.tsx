import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { motion } from "framer-motion";
import {
  Loader2,
  Save,
  FolderSearch,
  Database,
  Cloud,
  ScrollText,
  HardDriveDownload,
  FlaskConical,
} from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Group, GroupTitle, Row } from "@/components/ui/group";
import { settingsApi } from "@/lib/api";
import { LanguageSettings } from "@/components/settings/LanguageSettings";
import { ThemeSettings } from "@/components/settings/ThemeSettings";
import { WindowSettings } from "@/components/settings/WindowSettings";
import { AppVisibilitySettings } from "@/components/settings/AppVisibilitySettings";
import { TerminalSettings } from "@/components/settings/TerminalSettings";
import { DirectorySettings } from "@/components/settings/DirectorySettings";
import { ImportExportSection } from "@/components/settings/ImportExportSection";
import { BackupListSection } from "@/components/settings/BackupListSection";
import { WebdavSyncSection } from "@/components/settings/WebdavSyncSection";
import { AboutSection } from "@/components/settings/AboutSection";
import { ProxyTabContent } from "@/components/settings/ProxyTabContent";
import { ConnectivityCheckConfigPanel } from "@/components/usage/ConnectivityCheckConfigPanel";
import { LogConfigPanel } from "@/components/settings/LogConfigPanel";
import { AuthCenterPanel } from "@/components/settings/AuthCenterPanel";
import { CodexAuthSettings } from "@/components/settings/CodexAuthSettings";
import { useSettings } from "@/hooks/useSettings";
import { useImportExport } from "@/hooks/useImportExport";
import { useTranslation } from "react-i18next";
import type { SettingsFormState } from "@/hooks/useSettings";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImportSuccess?: () => void | Promise<void>;
  defaultTab?: string;
}

export function SettingsPage({
  open,
  onOpenChange,
  onImportSuccess,
  defaultTab = "general",
}: SettingsDialogProps) {
  const { t } = useTranslation();
  const {
    settings,
    isLoading,
    isSaving,
    isPortable,
    appConfigDir,
    resolvedDirs,
    updateSettings,
    updateDirectory,
    updateAppConfigDir,
    browseDirectory,
    browseAppConfigDir,
    resetDirectory,
    resetAppConfigDir,
    saveSettings,
    autoSaveSettings,
    requiresRestart,
    acknowledgeRestart,
  } = useSettings();

  const {
    selectedFile,
    status: importStatus,
    errorMessage,
    backupId,
    isImporting,
    selectImportFile,
    importConfig,
    exportConfig,
    clearSelection,
    resetStatus,
  } = useImportExport({ onImportSuccess });

  const [activeTab, setActiveTab] = useState<string>("general");
  const [showRestartPrompt, setShowRestartPrompt] = useState(false);
  const tabScrollContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      setActiveTab(defaultTab);
      resetStatus();
    }
  }, [open, resetStatus, defaultTab]);

  useEffect(() => {
    if (requiresRestart) {
      setShowRestartPrompt(true);
    }
  }, [requiresRestart]);

  useLayoutEffect(() => {
    if (tabScrollContainerRef.current) {
      tabScrollContainerRef.current.scrollTop = 0;
    }
  }, [activeTab]);

  const closeAfterSave = useCallback(() => {
    // 保存成功后关闭：不再重置语言，避免需要“保存两次”才生效
    acknowledgeRestart();
    clearSelection();
    resetStatus();
    onOpenChange(false);
  }, [acknowledgeRestart, clearSelection, onOpenChange, resetStatus]);

  const handleSave = useCallback(async () => {
    try {
      const result = await saveSettings(undefined, { silent: false });
      if (!result) return;
      if (result.requiresRestart) {
        setShowRestartPrompt(true);
        return;
      }
      closeAfterSave();
    } catch (error) {
      console.error("[SettingsPage] Failed to save settings", error);
    }
  }, [closeAfterSave, saveSettings]);

  const handleRestartLater = useCallback(() => {
    setShowRestartPrompt(false);
    closeAfterSave();
  }, [closeAfterSave]);

  const handleRestartNow = useCallback(async () => {
    setShowRestartPrompt(false);
    if (import.meta.env.DEV) {
      toast.success(t("settings.devModeRestartHint"), { closeButton: true });
      closeAfterSave();
      return;
    }

    try {
      await settingsApi.restart();
    } catch (error) {
      console.error("[SettingsPage] Failed to restart app", error);
      toast.error(t("settings.restartFailed"));
    } finally {
      closeAfterSave();
    }
  }, [closeAfterSave, t]);

  // 通用设置即时保存（无需手动点击）
  // 使用 autoSaveSettings 避免误触发系统 API（开机自启、Claude 插件等）
  // 返回保存是否成功：需要在保存成功后追加动作的调用方（如统一会话历史
  // 关闭后的备份还原）据此短路，其余调用方可忽略返回值。
  const handleAutoSave = useCallback(
    async (updates: Partial<SettingsFormState>): Promise<boolean> => {
      if (!settings) return false;
      // 乐观更新前捕获旧值：autoSaveSettings 发送的是全量表单状态，后端按
      // diff 触发副作用（如统一会话开关的 live 重写与历史迁移）。保存失败
      // 不回滚的话，失败的变更会滞留在表单里，被之后任意一次无关保存原样
      // 重放，绕过确认弹窗。
      const previousValues = Object.fromEntries(
        Object.keys(updates).map((key) => [
          key,
          settings[key as keyof SettingsFormState],
        ]),
      ) as Partial<SettingsFormState>;
      updateSettings(updates);
      try {
        await autoSaveSettings(updates);
        return true;
      } catch (error) {
        console.error("[SettingsPage] Failed to autosave settings", error);
        updateSettings(previousValues);
        toast.error(
          t("settings.saveFailedGeneric", {
            defaultValue: "保存失败，请重试",
          }),
        );
        return false;
      }
    },
    [autoSaveSettings, settings, t, updateSettings],
  );

  const isBusy = useMemo(() => isLoading && !settings, [isLoading, settings]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {isBusy ? (
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="size-5 animate-spin text-muted-foreground/40" />
        </div>
      ) : (
        <Tabs
          value={activeTab}
          onValueChange={setActiveTab}
          className="flex min-h-0 flex-1 flex-col"
        >
          <header
            className="sticky top-0 z-10 shrink-0 space-y-3 border-b border-border bg-background/80 px-7 py-4 backdrop-blur-xl"
            data-tauri-drag-region
          >
            <h1 className="text-xl font-semibold tracking-tight">
              {t("settings.title")}
            </h1>
            <TabsList className="no-drag grid w-full grid-cols-5">
              <TabsTrigger value="general">
                {t("settings.tabGeneral")}
              </TabsTrigger>
              <TabsTrigger value="proxy">{t("settings.tabProxy")}</TabsTrigger>
              <TabsTrigger value="auth">
                {t("settings.tabAuth", { defaultValue: "认证" })}
              </TabsTrigger>
              <TabsTrigger value="advanced">
                {t("settings.tabAdvanced")}
              </TabsTrigger>
              <TabsTrigger value="about">{t("common.about")}</TabsTrigger>
            </TabsList>
          </header>

          <div className="flex min-h-0 flex-1 flex-col">
            <div
              ref={tabScrollContainerRef}
              className="scroll-area flex-1 px-7 py-6 [&_[role=tabpanel]]:mx-auto [&_[role=tabpanel]]:w-full [&_[role=tabpanel]]:max-w-3xl"
            >
              <TabsContent value="general" className="mt-0 space-y-7">
                {settings ? (
                  <>
                    <section>
                      <GroupTitle>{t("settings.appearance")}</GroupTitle>
                      <Group>
                        <Row
                          label={t("settings.language")}
                          description={t("settings.languageHint")}
                          trailing={
                            <LanguageSettings
                              value={settings.language}
                              onChange={(lang) =>
                                handleAutoSave({ language: lang })
                              }
                            />
                          }
                        />
                        <Row
                          label={t("settings.theme")}
                          description={t("settings.themeHint")}
                          trailing={<ThemeSettings />}
                        />
                      </Group>
                    </section>

                    <AppVisibilitySettings
                      settings={settings}
                      onChange={handleAutoSave}
                    />

                    <section>
                      <GroupTitle>{t("settings.windowBehavior")}</GroupTitle>
                      <Group>
                        <WindowSettings
                          settings={settings}
                          onChange={handleAutoSave}
                        />
                        <TerminalSettings
                          value={settings.preferredTerminal}
                          onChange={(terminal) =>
                            handleAutoSave({ preferredTerminal: terminal })
                          }
                        />
                      </Group>
                    </section>

                    <CodexAuthSettings
                      settings={settings}
                      onChange={handleAutoSave}
                    />
                  </>
                ) : null}
              </TabsContent>

              <TabsContent value="proxy" className="space-y-6 mt-0 pb-4">
                {settings ? (
                  <ProxyTabContent
                    settings={settings}
                    onAutoSave={handleAutoSave}
                  />
                ) : null}
              </TabsContent>

              <TabsContent value="auth" className="space-y-6 mt-0 pb-4">
                <motion.div
                  initial={false}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.3 }}
                  className="space-y-6"
                >
                  <AuthCenterPanel />
                </motion.div>
              </TabsContent>

              <TabsContent value="advanced" className="space-y-6 mt-0 pb-4">
                {settings ? (
                  <motion.div
                    initial={false}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3 }}
                    className="space-y-4"
                  >
                    <Accordion
                      type="multiple"
                      defaultValue={[]}
                      className="w-full space-y-4"
                    >
                      <AccordionItem
                        value="directory"
                        className="overflow-hidden rounded-xl border border-border bg-card shadow-xs"
                      >
                        <AccordionTrigger>
                          <div className="flex items-center gap-3">
                            <FolderSearch className="size-[17px] text-muted-foreground" />
                            <div className="text-left">
                              <h3 className="text-base font-medium text-foreground">
                                {t("settings.advanced.configDir.title")}
                              </h3>
                              <p className="text-sm font-normal text-muted-foreground">
                                {t("settings.advanced.configDir.description")}
                              </p>
                            </div>
                          </div>
                        </AccordionTrigger>
                        <AccordionContent className="px-6 pb-6 pt-4 border-t border-border">
                          <DirectorySettings
                            appConfigDir={appConfigDir}
                            resolvedDirs={resolvedDirs}
                            onAppConfigChange={updateAppConfigDir}
                            onBrowseAppConfig={browseAppConfigDir}
                            onResetAppConfig={resetAppConfigDir}
                            claudeDir={settings.claudeConfigDir}
                            codexDir={settings.codexConfigDir}
                            geminiDir={settings.geminiConfigDir}
                            grokDir={settings.grokConfigDir}
                            opencodeDir={settings.opencodeConfigDir}
                            openclawDir={settings.openclawConfigDir}
                            hermesDir={settings.hermesConfigDir}
                            piDir={settings.piConfigDir}
                            onDirectoryChange={updateDirectory}
                            onBrowseDirectory={browseDirectory}
                            onResetDirectory={resetDirectory}
                          />
                        </AccordionContent>
                      </AccordionItem>

                      <AccordionItem
                        value="data"
                        className="overflow-hidden rounded-xl border border-border bg-card shadow-xs"
                      >
                        <AccordionTrigger>
                          <div className="flex items-center gap-3">
                            <Database className="size-[17px] text-muted-foreground" />
                            <div className="text-left">
                              <h3 className="text-base font-medium text-foreground">
                                {t("settings.advanced.data.title")}
                              </h3>
                              <p className="text-sm font-normal text-muted-foreground">
                                {t("settings.advanced.data.description")}
                              </p>
                            </div>
                          </div>
                        </AccordionTrigger>
                        <AccordionContent className="px-6 pb-6 pt-4 border-t border-border">
                          <ImportExportSection
                            status={importStatus}
                            selectedFile={selectedFile}
                            errorMessage={errorMessage}
                            backupId={backupId}
                            isImporting={isImporting}
                            onSelectFile={selectImportFile}
                            onImport={importConfig}
                            onExport={exportConfig}
                            onClear={clearSelection}
                          />
                        </AccordionContent>
                      </AccordionItem>

                      <AccordionItem
                        value="backup"
                        className="overflow-hidden rounded-xl border border-border bg-card shadow-xs"
                      >
                        <AccordionTrigger>
                          <div className="flex items-center gap-3">
                            <HardDriveDownload className="h-5 w-5 text-warning" />
                            <div className="text-left">
                              <h3 className="text-base font-medium text-foreground">
                                {t("settings.advanced.backup.title", {
                                  defaultValue: "Backup & Restore",
                                })}
                              </h3>
                              <p className="text-sm font-normal text-muted-foreground">
                                {t("settings.advanced.backup.description", {
                                  defaultValue:
                                    "Manage automatic backups, view and restore database snapshots",
                                })}
                              </p>
                            </div>
                          </div>
                        </AccordionTrigger>
                        <AccordionContent className="px-6 pb-6 pt-4 border-t border-border">
                          <BackupListSection
                            backupIntervalHours={settings.backupIntervalHours}
                            backupRetainCount={settings.backupRetainCount}
                            onSettingsChange={(updates) =>
                              handleAutoSave(updates)
                            }
                          />
                        </AccordionContent>
                      </AccordionItem>

                      <AccordionItem
                        value="cloudSync"
                        className="overflow-hidden rounded-xl border border-border bg-card shadow-xs"
                      >
                        <AccordionTrigger>
                          <div className="flex items-center gap-3">
                            <Cloud className="size-[17px] text-muted-foreground" />
                            <div className="text-left">
                              <h3 className="text-base font-medium text-foreground">
                                {t("settings.advanced.cloudSync.title")}
                              </h3>
                              <p className="text-sm font-normal text-muted-foreground">
                                {t("settings.advanced.cloudSync.description")}
                              </p>
                            </div>
                          </div>
                        </AccordionTrigger>
                        <AccordionContent className="px-6 pb-6 pt-4 border-t border-border">
                          <WebdavSyncSection
                            config={settings?.webdavSync}
                            s3Config={settings?.s3Sync}
                            settings={settings}
                            onAutoSave={handleAutoSave}
                          />
                        </AccordionContent>
                      </AccordionItem>

                      <AccordionItem
                        value="connectivityCheck"
                        className="overflow-hidden rounded-xl border border-border bg-card shadow-xs"
                      >
                        <AccordionTrigger>
                          <div className="flex items-center gap-3">
                            <FlaskConical className="h-5 w-5 text-success" />
                            <div className="text-left">
                              <h3 className="text-base font-medium text-foreground">
                                {t("settings.advanced.connectivityCheck.title")}
                              </h3>
                              <p className="text-sm font-normal text-muted-foreground">
                                {t(
                                  "settings.advanced.connectivityCheck.description",
                                )}
                              </p>
                            </div>
                          </div>
                        </AccordionTrigger>
                        <AccordionContent className="px-6 pb-6 pt-4 border-t border-border">
                          <ConnectivityCheckConfigPanel />
                        </AccordionContent>
                      </AccordionItem>

                      <AccordionItem
                        value="logConfig"
                        className="overflow-hidden rounded-xl border border-border bg-card shadow-xs"
                      >
                        <AccordionTrigger>
                          <div className="flex items-center gap-3">
                            <ScrollText className="size-[17px] text-muted-foreground" />
                            <div className="text-left">
                              <h3 className="text-base font-medium text-foreground">
                                {t("settings.advanced.logConfig.title")}
                              </h3>
                              <p className="text-sm font-normal text-muted-foreground">
                                {t("settings.advanced.logConfig.description")}
                              </p>
                            </div>
                          </div>
                        </AccordionTrigger>
                        <AccordionContent className="px-6 pb-6 pt-4 border-t border-border">
                          <LogConfigPanel />
                        </AccordionContent>
                      </AccordionItem>
                    </Accordion>
                  </motion.div>
                ) : null}
              </TabsContent>

              <TabsContent value="about" className="mt-0">
                <AboutSection isPortable={isPortable} />
              </TabsContent>
            </div>

            {activeTab === "advanced" && settings && (
              <div
                className="flex-shrink-0 pt-4 border-t border-border"
                style={{ backgroundColor: "hsl(var(--background))" }}
              >
                <div className="px-6 flex items-center justify-end gap-3">
                  <Button onClick={handleSave} disabled={isSaving}>
                    {isSaving ? (
                      <span className="inline-flex items-center gap-2">
                        <Loader2 className="h-4 w-4 animate-spin" />
                        {t("settings.saving")}
                      </span>
                    ) : (
                      <>
                        <Save className="mr-2 h-4 w-4" />
                        {t("common.save")}
                      </>
                    )}
                  </Button>
                </div>
              </div>
            )}
          </div>
        </Tabs>
      )}

      <Dialog
        open={showRestartPrompt}
        onOpenChange={(open) => !open && handleRestartLater()}
      >
        <DialogContent layer="alert" className="max-w-md border-border">
          <DialogHeader>
            <DialogTitle>{t("settings.restartRequired")}</DialogTitle>
          </DialogHeader>
          <div className="px-6">
            <p className="text-sm text-muted-foreground">
              {t("settings.restartRequiredMessage")}
            </p>
          </div>
          <DialogFooter>
            <Button
              variant="ghost"
              onClick={handleRestartLater}
              className="hover:bg-muted"
            >
              {t("settings.restartLater")}
            </Button>
            <Button
              onClick={handleRestartNow}
              className="bg-primary hover:bg-primary/90"
            >
              {t("settings.restartNow")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
