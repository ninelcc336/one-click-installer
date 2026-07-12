import { chooseInstallDir, closeInstaller, getPreflight, launchApp, listenToInstallEvents, openLogFolder, startInstall } from "./api";
import { render } from "./ui/render";
import type { InstallEvent, UiState } from "./ui/types";

export function renderReadyScreen(root: HTMLElement): void {
  root.innerHTML = `<section class="shell"><h1>Codex 与 CC Switch</h1><button disabled>安装器准备就绪</button></section>`;
}

export function mountInstaller(root: HTMLElement): void {
  let state: UiState = { screen: "welcome", preflight: null, preflightLoading: true, selectedApps: [], installDir: "", events: [], error: null };
  let stopListening: (() => void) | undefined;

  const update = (next: Partial<UiState>) => {
    state = { ...state, ...next };
    render(state, root);
  };

  const beginInstall = async () => {
    update({ screen: "progress", events: [], error: null });
    try {
      stopListening = await listenToInstallEvents((event) => {
        state = { ...state, events: [...state.events, event] };
        render(state, root);
        const finishedApps = new Set(
          state.events
            .filter((item) => item.app_id && ["Completed", "Skipped"].includes(item.phase))
            .map((item) => item.app_id),
        );
        if (state.selectedApps.every((app) => finishedApps.has(app))) {
          update({ screen: "complete" });
        }
      });
      await startInstall(state.selectedApps, state.installDir);
    } catch (error) {
      update({ screen: "failure", error: readableError(error) });
    }
  };

  const loadPreflight = async () => {
    update({ preflightLoading: true, error: null });
    try {
      const preflight = await getPreflight();
      update({
        preflight,
        preflightLoading: false,
        installDir: preflight.install_dir,
        selectedApps: preflight.apps.filter((app) => app.available).map((app) => app.app_id),
      });
    } catch (error) {
      update({ preflightLoading: false, error: readableError(error) });
    }
  };

  root.addEventListener("click", async (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const action = target.dataset.action;
    if (action === "start") {
      if (!state.preflight) {
        await loadPreflight();
      }
      update({ screen: "preflight" });
    } else if (action === "install" || action === "retry") {
      await beginInstall();
    } else if (action === "back") {
      update({ screen: "welcome", preflight: null, preflightLoading: false, selectedApps: [], installDir: "" });
    } else if (action === "choose-dir") {
      try {
        const installDir = await chooseInstallDir();
        update({ installDir });
      } catch {
        // 用户取消目录选择时保持当前目录不变。
      }
    } else if (action === "logs") {
      await openLogFolder();
    } else if (action === "launch-codex" || action === "launch-cc-switch") {
      try {
        await launchApp(action === "launch-codex" ? "codex" : "cc-switch");
      } catch (error) {
        update({ screen: "failure", error: readableError(error) });
      }
    } else if (action === "finish" || action === "exit" || action === "cancel") {
      stopListening?.();
      stopListening = undefined;
      if (action === "exit" || action === "cancel") {
        await closeInstaller();
      } else {
        update({ screen: "welcome" });
      }
    }
  });

  root.addEventListener("change", (event) => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement) || target.dataset.appId === undefined) return;
    const appId = target.dataset.appId as UiState["selectedApps"][number];
    const selectedApps = target.checked
      ? [...new Set([...state.selectedApps, appId])]
      : state.selectedApps.filter((item) => item !== appId);
    update({ selectedApps });
  });

  void loadPreflight();
  render(state, root);
}

function readableError(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message) return message;
  }
  return "安装器暂时无法继续，请稍后重试。";
}
