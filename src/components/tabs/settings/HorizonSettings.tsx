import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Icon } from "@iconify/react";
import { toast } from "react-hot-toast";
import { useTranslation } from "react-i18next";
import { Button } from "../../ui/buttons/Button";
import { SettingsSection } from "../../ui/settings/SettingsSection";
import { SettingRow } from "../../ui/settings/SettingRow";
import { useSettingsConfig, useSettingsKeywords } from "./settings-context";
import {
  configureHorizon,
  launchHorizon,
  stopHorizon,
  type HorizonProcessStatus,
} from "../../../services/horizon-service";

export function HorizonSettings() {
  const { t } = useTranslation();
  const keywords = useSettingsKeywords();
  const { tempConfig, setTempConfig, saving } = useSettingsConfig();
  const [world, setWorld] = useState("");
  const [renderDistance, setRenderDistance] = useState(12);
  const [process, setProcess] = useState<HorizonProcessStatus | null>(null);
  const running = process?.state === "running" || process?.state === "stopping";

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    listen<HorizonProcessStatus>("horizon-process-state", (event) => {
      if (mounted) setProcess(event.payload);
    }).then((dispose) => {
      if (mounted) unlisten = dispose;
      else dispose();
    }).catch((error) => console.error("Could not subscribe to Horizon process state:", error));
    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  const chooseInstallation = async () => {
    try {
      const executable = await open({ multiple: false, directory: false });
      if (typeof executable !== "string") return;
      const resources = await open({ multiple: false, directory: true });
      if (typeof resources !== "string") return;
      await configureHorizon(executable, resources);
      if (tempConfig) {
        setTempConfig({
          ...tempConfig,
          horizon_executable: executable,
          horizon_resources: resources,
        });
      }
      toast.success(t("settings.horizon.configured"));
    } catch (error) {
      console.error("Failed to configure Horizon:", error);
      toast.error(String(error));
    }
  };

  const start = async () => {
    try {
      const status = await launchHorizon(world.trim() || null, renderDistance);
      setProcess(status);
      toast.success(t("settings.horizon.started"));
    } catch (error) {
      console.error("Failed to launch Horizon:", error);
      toast.error(String(error));
    }
  };

  return (
    <SettingsSection
      id="settings-section-horizon"
      title={t("settings.horizon.title")}
      icon="solar:planet-3-bold"
      description={t("settings.horizon.description")}
      keywords={keywords("settings.horizon.title", "horizon", "native", "rust engine")}
    >
      <SettingRow
        label={t("settings.horizon.installation")}
        description={t("settings.horizon.installation_description")}
        searchKeywords={keywords("settings.horizon.installation", "horizon", "executable", "resources")}
      >
        <div className="flex flex-col gap-2 min-w-0 w-full">
          <span className="text-xs text-white/60 truncate">
            {tempConfig?.horizon_executable || t("settings.horizon.not_configured")}
          </span>
          <Button variant="ghost" disabled={saving || running} onClick={chooseInstallation}>
            <Icon icon="solar:folder-open-bold" className="mr-2 h-4 w-4" />
            {t("settings.horizon.choose")}
          </Button>
        </div>
      </SettingRow>
      <SettingRow
        label={t("settings.horizon.world")}
        description={t("settings.horizon.world_description")}
        searchKeywords={keywords("settings.horizon.world", "world", "save")}
        disabled={running}
      >
        <input
          value={world}
          onChange={(event) => setWorld(event.target.value)}
          placeholder={t("settings.horizon.default_world")}
          maxLength={64}
          className="w-full p-3 rounded-md bg-black/40 border border-[#ffffff20] text-white placeholder-white/40 font-minecraft focus:outline-none focus:ring-2 focus:ring-white/30"
          disabled={running}
        />
      </SettingRow>
      <SettingRow
        label={t("settings.horizon.render_distance")}
        description={t("settings.horizon.render_distance_description")}
        searchKeywords={keywords("settings.horizon.render_distance", "distance", "chunks")}
        disabled={running}
      >
        <input
          type="number"
          min={2}
          max={64}
          value={renderDistance}
          onChange={(event) => setRenderDistance(Math.max(2, Math.min(64, Number(event.target.value) || 2)))}
          className="w-24 p-3 rounded-md bg-black/40 border border-[#ffffff20] text-white font-minecraft focus:outline-none focus:ring-2 focus:ring-white/30"
          disabled={running}
        />
      </SettingRow>
      <SettingRow
        label={t("settings.horizon.process")}
        description={t("settings.horizon.process_description")}
        searchKeywords={keywords("settings.horizon.process", "launch", "stop", "process")}
      >
        <div className="flex items-center gap-2">
          <Button variant="secondary" disabled={saving || running} onClick={start}>
            {t("settings.horizon.launch")}
          </Button>
          <Button
            variant="ghost"
            disabled={!running || !process}
            onClick={() => process && stopHorizon(process.process_id).catch((error) => toast.error(String(error)))}
          >
            {t("settings.horizon.stop")}
          </Button>
          {process && <span className="text-xs text-white/60">{t(`settings.horizon.state_${process.state}`)}</span>}
        </div>
      </SettingRow>
    </SettingsSection>
  );
}
