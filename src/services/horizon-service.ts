import { invoke } from "@tauri-apps/api/core";

export interface HorizonProcessStatus {
  process_id: string;
  pid: number | null;
  state: "running" | "stopping" | "exited" | "failed";
  exit_code: number | null;
}

export function configureHorizon(executablePath: string, resourcesPath: string): Promise<void> {
  return invoke("configure_horizon", { executablePath, resourcesPath });
}

export function launchHorizon(world: string | null, renderDistance: number | null): Promise<HorizonProcessStatus> {
  return invoke("launch_horizon", { world, renderDistance });
}

export function stopHorizon(processId: string): Promise<void> {
  return invoke("stop_horizon", { processId });
}

export function getHorizonProcess(processId: string): Promise<HorizonProcessStatus> {
  return invoke("get_horizon_process", { processId });
}
