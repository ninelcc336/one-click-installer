import { closeInstaller, getPreflight, launchApp, listenToInstallEvents, openLogFolder, startInstall } from "./api";
import { render } from "./ui/render";
import type { InstallEvent, UiState } from "./ui/types";

export function renderReadyScreen(root: HTMLElement): void {
  root.innerHTML = `<section class="shell"><h1>Codex 与 CC Switch</h1><button disabled>安装器准备就绪</button></section>`;
}

export function mountInstaller(root: HTMLElement): void {
  let state: UiState = { screen: "welcome", preflight: null, events: [], error: null };
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
        if (event.phase === "Completed" && state.events.filter((item) => item.phase === "Completed").length >= 2) {
          update({ screen: "complete" });
        }
      });
      await startInstall();
    } catch (error) {
      update({ screen: "failure", error: readableError(error) });
    }
  };

  root.addEventListener("click", async (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    const action = target.dataset.action;
    if (action === "start") {
      update({ screen: "preflight", error: null });
      try {
        update({ preflight: await getPreflight() });
      } catch (error) {
        update({ screen: "failure", error: readableError(error) });
      }
    } else if (action === "install" || action === "retry") {
      await beginInstall();
    } else if (action === "back") {
      update({ screen: "welcome", preflight: null });
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

  render(state, root);
}

function readableError(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  return "安装器暂时无法继续，请稍后重试。";
}
