export function renderReadyScreen(root: HTMLElement): void {
  root.innerHTML = `
    <section class="shell" aria-labelledby="title">
      <div class="brand-mark" aria-hidden="true">✦</div>
      <p class="eyebrow">安装助手</p>
      <h1 id="title">Codex 与 CC Switch</h1>
      <p class="subtitle">准备好后，我们会自动检测电脑并完成安装。</p>
      <button class="primary-button" type="button" disabled>安装器准备就绪</button>
    </section>
  `;
}
