# Installer Entry and Back Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 区分欢迎页和预检页的主按钮文案，并保留返回时已有的预检状态。

**Architecture:** 继续使用 `UiState` 驱动渲染。新增一个只用于欢迎页的文案键；返回操作只切换 `screen`，不清除已经成功获取的 `preflight` 和选择状态。

**Tech Stack:** TypeScript、Vitest、JSDOM、Vite、原生 CSS。

## Global Constraints

- 欢迎页不执行实际安装。
- 预检页的“开始安装”按钮继续触发安装。
- 返回欢迎页不得把已有检测结果转换为错误状态。
- 不改变 Tauri command 和 Rust 后端接口。

---

### Task 1: 区分入口文案并修复返回状态

**Files:**
- Modify: `src/ui/copy.ts`
- Modify: `src/ui/render.ts`
- Modify: `src/app.ts`

**Interfaces:**
- Consumes: `UiState.screen`, `UiState.preflight` and existing click actions.
- Produces: welcome-only `enterFlow` copy and state-preserving `back` behavior.

- [ ] **Step 1: 为欢迎页增加独立文案**

在 `src/ui/copy.ts` 中增加：

```ts
enterFlow: "进入安装流程",
```

并让 `welcomePage()` 使用 `copy.enterFlow`，`preflightPage()` 保持使用 `copy.start`。

- [ ] **Step 2: 保留返回时的状态**

将 `src/app.ts` 中的返回处理从清空状态：

```ts
update({ screen: "welcome", preflight: null, preflightLoading: false, selectedApps: [], installDir: "" });
```

改为：

```ts
update({ screen: "welcome" });
```

这样欢迎页会继续使用已有预检结果，重新进入时也会保留应用选择和安装目录。

### Task 2: 增加页面状态回归测试

**Files:**
- Modify: `src/ui/render.test.ts`

**Interfaces:**
- Consumes: `render(state, root)`。
- Produces: 文案和状态显示的回归覆盖。

- [ ] **Step 1: 测试欢迎页入口文案**

渲染带有预检结果的 `welcome` 状态，断言包含“进入安装流程”，且不包含“开始安装”。

- [ ] **Step 2: 测试预检页仍可开始安装**

渲染可用应用的 `preflight` 状态，断言安装按钮仍包含“开始安装”且没有 `disabled` 属性。

- [ ] **Step 3: 运行前端测试和构建**

```powershell
npm run test
npm run build
```

预期：全部前端测试通过，Vite 构建成功。

### Task 3: 完成验证

**Files:**
- None

- [ ] **Step 1: 检查差异**

确认只修改入口文案、返回状态和对应测试，不改变后端检测逻辑。
