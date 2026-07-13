# CC Switch 检测与主按钮体验优化设计

## 目标

修复 Windows 上已安装 CC Switch 却显示“未安装”的问题，并让“开始安装”按钮在可点击时具备明确的主操作视觉，在不可点击时保留清晰的禁用反馈。

## 方案

### CC Switch 检测

现有版本清单使用了 `Programs/CC Switch/CC Switch.exe`，实际安装文件为 `Programs/CC Switch/cc-switch.exe`。检测逻辑仍保留通用适配器结构，但将 Windows 清单路径改为真实文件名，并让候选路径覆盖当前用户程序目录、系统程序目录和当前安装目录。检测结果继续通过 `InstalledApp` 返回，启动逻辑复用检测到的真实可执行文件路径。

### 主按钮视觉

继续使用现有 `.primary-button` 组件，不引入新的前端依赖。启用态使用高对比蓝色背景、明确的阴影和指针反馈；悬停、按下和键盘焦点分别提供反馈；禁用态只在 HTML `disabled` 生效时使用灰色和禁止光标。按钮保持现有布局与文案，避免改变向导流程。

## 测试与验收

- Rust 单元测试覆盖带连字符的 CC Switch 可执行文件路径。
- 前端渲染测试确认“开始安装”按钮在可用状态下没有 `disabled` 属性，在不可用状态下保留 `disabled` 属性。
- `npm run test`、`cargo test --manifest-path src-tauri/Cargo.toml` 和 `npm run build` 全部通过。
