// @vitest-environment jsdom

import { describe, expect, it, vi } from "vitest";
import { mountInstaller, renderReadyScreen } from "./app";

vi.mock("./api", () => ({
  chooseInstallDir: vi.fn(),
  closeInstaller: vi.fn(),
  getPreflight: vi.fn().mockResolvedValue({
    target: "windows-x64",
    free_space_bytes: 1024 * 1024 * 1024,
    can_write: true,
    install_dir: "C:/Users/test/AppData/Local/Programs",
    install_path_note: "CC Switch 使用官方安装器。",
    apps: [
      { app_id: "codex", available: true, installed: false, message: "可以安装" },
      { app_id: "cc-switch", available: true, installed: false, message: "可以安装" },
    ],
  }),
  launchApp: vi.fn(),
  listenToInstallEvents: vi.fn().mockResolvedValue(() => {}),
  openLogFolder: vi.fn(),
  startInstall: vi.fn(),
}));

describe("初始安装器页面", () => {
  it("显示应用名称和准备状态", () => {
    const root = document.createElement("main");

    renderReadyScreen(root);

    expect(root.querySelector("h1")?.textContent).toBe("Codex 与 CC Switch");
    expect(root.querySelector("button")?.textContent).toBe("安装器准备就绪");
    expect(root.querySelector("button")?.hasAttribute("disabled")).toBe(true);
  });

  it("从选择安装内容页返回时保留检测结果", async () => {
    const root = document.createElement("main");
    mountInstaller(root);

    await vi.waitFor(() => {
      expect(root.querySelector('[data-action="start"]')).not.toBeNull();
    });
    root.querySelector<HTMLButtonElement>('[data-action="start"]')?.click();
    await vi.waitFor(() => {
      expect(root.querySelector('[data-action="install"]')).not.toBeNull();
    });

    root.querySelector<HTMLButtonElement>('[data-action="back"]')?.click();
    await vi.waitFor(() => {
      expect(root.querySelector('[data-action="start"]')?.textContent).toBe("进入安装流程");
    });

    expect(root.textContent).not.toContain("暂时无法检测电脑状态");
    expect(root.textContent).toContain("Codex");
    expect(root.textContent).toContain("CC Switch");
  });
});
