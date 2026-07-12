// @vitest-environment jsdom

import { describe, expect, it } from "vitest";
import { renderReadyScreen } from "./app";

describe("初始安装器页面", () => {
  it("显示应用名称和准备状态", () => {
    const root = document.createElement("main");

    renderReadyScreen(root);

    expect(root.querySelector("h1")?.textContent).toBe("Codex 与 CC Switch");
    expect(root.querySelector("button")?.textContent).toBe("安装器准备就绪");
    expect(root.querySelector("button")?.hasAttribute("disabled")).toBe(true);
  });
});
