import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { AppId, InstallEvent, PreflightResult } from "./ui/types";

export function getPreflight(): Promise<PreflightResult> {
  return invoke<PreflightResult>("get_preflight");
}

export function startInstall(selectedApps: AppId[], installDir: string): Promise<void> {
  return invoke("start_install", { selectedApps, installDir });
}

export function openLogFolder(): Promise<string> {
  return invoke<string>("open_log_folder");
}

export function chooseInstallDir(): Promise<string> {
  return invoke<string>("choose_install_dir");
}

export function launchApp(appId: "codex" | "cc-switch"): Promise<void> {
  return invoke("launch_app", { appId });
}

export function closeInstaller(): Promise<void> {
  return getCurrentWindow().close();
}

export function listenToInstallEvents(handler: (event: InstallEvent) => void): Promise<UnlistenFn> {
  return listen<InstallEvent>("install-event", (event) => handler(event.payload));
}
