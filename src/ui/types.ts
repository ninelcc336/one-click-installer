export type Screen = "welcome" | "preflight" | "progress" | "complete" | "failure";

export type AppId = "Codex" | "CC Switch";

export interface PreflightApp {
  app_id: AppId;
  available: boolean;
  installed: boolean;
  message: string;
}

export interface PreflightResult {
  target: string;
  free_space_bytes: number;
  can_write: boolean;
  apps: PreflightApp[];
}

export interface InstallEvent {
  app_id: AppId | null;
  phase: string;
  downloaded: number | null;
  total: number | null;
  message_key: string;
}

export interface UiState {
  screen: Screen;
  preflight: PreflightResult | null;
  events: InstallEvent[];
  error: string | null;
}
