// @vitest-environment jsdom

import { describe, expect, it } from "vitest";
import { render } from "./render";
import type { UiState } from "./types";

describe("安装向导页面", () => {
  it("没有安装包清单时禁用开始按钮并给出解释", () => {
    const root = document.createElement("main");
    const state: UiState = {
      screen: "preflight",
      preflight: {
        target: "windows-x64",
        free_space_bytes: 1024 * 1024 * 1024,
        can_write: true,
        install_dir: "C:/Users/test/AppData/Local/Programs",
        install_path_note: "Codex 的 Microsoft Store 版本由系统管理安装位置。",
        apps: [
          { app_id: "codex", available: false, installed: false, message: "等待核验安装包信息" },
          { app_id: "cc-switch", available: false, installed: false, message: "等待核验安装包信息" },
        ],
      },
      selectedApps: [],
      installDir: "C:/Users/test/AppData/Local/Programs",
      preflightLoading: false,
      events: [],
      error: null,
    };

    render(state, root);

    expect(root.textContent).toContain("当前还没有可用的安装包清单");
    expect(root.querySelector('[data-action="install"]')?.hasAttribute("disabled")).toBe(true);
  });

  it("进度页显示两个应用", () => {
    const root = document.createElement("main");
    render({ screen: "progress", preflight: null, preflightLoading: false, selectedApps: ["codex", "cc-switch"], installDir: "", events: [], error: null }, root);

    expect(root.textContent).toContain("Codex");
    expect(root.textContent).toContain("CC Switch");
  });
});
