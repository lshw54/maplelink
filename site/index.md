---
layout: page
title: MapleLink
titleTemplate: 《新楓之谷》的第三方登入啟動器
---

<script setup>
import { withBase } from "vitepress";
</script>

<div class="landing">

<section class="landing-hero">
  <div>
    <span class="landing-eyebrow">第三方 · 開放原始碼 · 非遊戲橘子官方</span>
    <h1>《新楓之谷》的<br>登入啟動器</h1>
    <p class="landing-lead">更快捷的帳號管理，更好的登入體驗。</p>
    <LatestRelease product="maplelink" variant="hero" />
    <p class="landing-fine">只從本站或 GitHub Releases 下載。<a :href="withBase('/download#verify')">核對 SHA256</a> · <a :href="withBase('/guide/')">新手教學</a><br>也玩其他橘子遊戲？我們同時維護原版 <a href="https://github.com/pungin/Beanfun">Beanfun</a> 啟動器。</p>
  </div>
  <div class="landing-window"><AppWindow /></div>
</section>

<section class="landing-help">
  <div>
    <h2>開始使用</h2>
    <ul>
      <li><a :href="withBase('/guide/')">新手教學</a>：下載、解壓、登入、取 OTP，約五分鐘</li>
      <li>需要 Windows 10 / 11（64 位元）及 WebView2</li>
      <li>不需要安裝，整個資料夾放哪裏都可以</li>
    </ul>
  </div>
  <div>
    <h2>遇到問題</h2>
    <ul>
      <li><a :href="withBase('/faq#webview2')">打不開、白屏、閃退</a>：安裝 WebView2</li>
      <li><a :href="withBase('/faq')">SmartScreen 警告、防毒誤判、加速器認不到</a></li>
      <li>其他問題到 <a href="https://github.com/lshw54/maplelink/issues">GitHub Issues</a> 回報</li>
    </ul>
  </div>
  <div>
    <h2>下載與更新</h2>
    <ul>
      <li>只從本站或 <a href="https://github.com/lshw54/maplelink/releases">GitHub Releases</a> 下載</li>
      <li>執行前<a :href="withBase('/download#verify')">核對 SHA256</a>，不一致就刪除</li>
      <li>已在使用的話，程式會自動更新</li>
    </ul>
  </div>
</section>

<section class="landing-legal">
  <p>MapleLink 與 <a href="https://github.com/pungin/Beanfun">Beanfun</a> 均為第三方開放原始碼軟體，由同一批人並行維護，與遊戲橘子（Gamania）、beanfun! 及 Nexon 沒有任何關聯，也不是它們的官方產品。使用第三方啟動器可能違反遊戲服務條款，請自行評估風險；因使用本軟體造成的任何帳號或財物損失，開發團隊不承擔責任。</p>
  <p>記住的帳號密碼只儲存在你的電腦，以 Windows DPAPI 加密，不會傳送給開發團隊。程式碼以 <a href="https://github.com/lshw54/maplelink/blob/main/LICENSE">MIT 授權</a>公開於 GitHub，任何人都可以查看與核對。</p>
</section>

</div>
