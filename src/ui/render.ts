import { copy } from "./copy";
import type { AppId, InstallEvent, UiState } from "./types";

export function render(state: UiState, root: HTMLElement): void {
  root.innerHTML = pageFor(state);
}

function pageFor(state: UiState): string {
  switch (state.screen) {
    case "welcome":
      return welcomePage(state);
    case "preflight":
      return preflightPage(state);
    case "progress":
      return progressPage(state);
    case "complete":
      return completePage(state);
    case "failure":
      return failurePage(state);
  }
}

function frame(content: string, title: string, subtitle: string): string {
  return `
    <section class="shell" aria-labelledby="title">
      <div class="brand-mark" aria-hidden="true">✦</div>
      <p class="eyebrow">${copy.eyebrow}</p>
      <h1 id="title">${title}</h1>
      <p class="subtitle">${subtitle}</p>
      ${content}
    </section>
  `;
}

function welcomePage(state: UiState): string {
  const detection = state.preflightLoading
    ? `<div class="detecting-card"><span class="loading-dot" aria-hidden="true"></span>${copy.detecting}</div>`
    : state.preflight
      ? `<div class="panel compact-panel"><div class="status-summary">${state.preflight.apps.map((app) => `<div class="summary-item"><span class="status-dot ${app.installed ? "ok" : "neutral"}" aria-hidden="true"></span><span>${appLabel(app.app_id)}</span><strong>${app.installed ? "已安装" : copy.notInstalled}</strong></div>`).join("")}</div></div>`
      : `<p class="notice error">${state.error ?? "暂时无法检测电脑状态"}</p>`;
  const disabled = state.preflightLoading || !state.preflight;
  return frame(
    `${detection}<div class="actions"><button class="primary-button" data-action="start" type="button" ${disabled ? "disabled" : ""}>${copy.enterFlow}</button></div>`,
    copy.title,
    copy.subtitle,
  );
}

function preflightPage(state: UiState): string {
  const preflight = state.preflight;
  const apps = preflight?.apps ?? [];
  const unavailable = apps.some((app) => !app.available);
  const selectedUnavailable = state.selectedApps.some((appId) => apps.some((app) => app.app_id === appId && !app.available));
  const permissionBlocked = preflight ? !preflight.can_write : true;
  const rows = apps
    .map(
      (app) => `
        <label class="check-row ${app.available ? "" : "muted"}">
          <input class="app-checkbox" type="checkbox" data-app-id="${app.app_id}" ${state.selectedApps.includes(app.app_id) ? "checked" : ""} ${app.available ? "" : "disabled"} />
          <span class="status-dot ${app.installed ? "ok" : app.available ? "neutral" : "warn"}" aria-hidden="true"></span>
          <span class="check-name">${appLabel(app.app_id)}</span>
          <span class="check-message">${app.message}</span>
        </label>
      `,
    )
    .join("");
  const notice = unavailable
    ? `<p class="notice warning">${copy.noPackage}</p>`
    : permissionBlocked
      ? `<p class="notice warning">${copy.permission}</p>`
      : "";
  const canContinue = !selectedUnavailable && !permissionBlocked && state.selectedApps.length > 0;
  const selectionNotice = state.selectedApps.length === 0
    ? `<p class="notice warning">请至少选择一个应用。</p>`
    : "";

  return frame(
    `<div class="panel"><div class="check-list">${rows}</div>${notice}</div><div class="path-picker"><div><span class="path-label">安装位置</span><span class="path-value">${state.installDir || "未选择"}</span></div><button class="secondary-button" data-action="choose-dir" type="button">浏览</button></div><p class="path-note">${preflight?.install_path_note ?? ""}</p>${selectionNotice}<div class="actions"><button class="primary-button" data-action="install" type="button" ${canContinue ? "" : "disabled"}>${copy.start}</button><button class="text-button" data-action="back" type="button">${copy.back}</button></div>`,
    copy.checkTitle,
    preflight ? `系统：${preflight.target} · 可用空间：${formatBytes(preflight.free_space_bytes)}` : copy.checkSubtitle,
  );
}

function progressPage(state: UiState): string {
  const latest = latestByApp(state.events);
  const rows = state.selectedApps
    .map((app) => {
      const event = latest.get(app);
      const percent = event?.total ? Math.min(100, Math.round(((event.downloaded ?? 0) / event.total) * 100)) : event?.phase === "Completed" || event?.phase === "Skipped" ? 100 : 0;
      return `<div class="progress-row"><div class="progress-heading"><span>${appLabel(app)}</span><span>${eventLabel(event)}</span></div><div class="progress-track"><div class="progress-value" style="width:${percent}%"></div></div></div>`;
    })
    .join("");
  return frame(`<div class="panel progress-list">${rows}</div><button class="text-button" data-action="cancel" type="button">${copy.exit}</button>`, copy.progressTitle, copy.progressSubtitle);
}

function completePage(state: UiState): string {
  const launchButtons = state.selectedApps
    .map((app) => app === "codex"
      ? `<button class="primary-button" data-action="launch-codex" type="button">${copy.openCodex}</button>`
      : `<button class="secondary-button" data-action="launch-cc-switch" type="button">${copy.openCcSwitch}</button>`)
    .join("");
  return frame(`<div class="success-card"><span class="success-icon" aria-hidden="true">✓</span><p>首次使用时，请在应用内完成登录。</p></div><div class="actions">${launchButtons}<button class="text-button" data-action="finish" type="button">${copy.finish}</button></div>`, copy.completeTitle, copy.completeSubtitle);
}

function failurePage(state: UiState): string {
  return frame(`<p class="notice error">${state.error ?? "发生了未知错误"}</p><div class="actions"><button class="primary-button" data-action="retry" type="button">${copy.retry}</button><button class="text-button" data-action="logs" type="button">${copy.exportLog}</button></div>`, copy.failureTitle, copy.failureSubtitle);
}

function latestByApp(events: InstallEvent[]): Map<string, InstallEvent> {
  const result = new Map<string, InstallEvent>();
  for (const event of events) {
    if (event.app_id) result.set(event.app_id, event);
  }
  return result;
}

function eventLabel(event: InstallEvent | undefined): string {
  if (!event) return "等待中";
  if (event.downloaded !== null && event.total) return `${Math.round((event.downloaded / event.total) * 100)}%`;
  if (event.phase === "Completed") return "已完成";
  if (event.phase === "Skipped") return "已保留";
  if (event.phase === "Failed") return "失败";
  return "处理中";
}

function formatBytes(bytes: number): string {
  if (bytes > 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  return `${Math.max(1, Math.round(bytes / 1024 ** 2))} MB`;
}

function appLabel(appId: AppId): string {
  return appId === "codex" ? "Codex" : "CC Switch";
}
