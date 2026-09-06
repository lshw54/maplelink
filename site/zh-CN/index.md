---
layout: page
title: MapleLink
titleTemplate: 《新枫之谷》的第三方登录启动器
---

<script setup>
import { withBase } from "vitepress";
</script>

<div class="landing">

<section class="landing-hero">
  <div>
    <span class="landing-eyebrow">第三方 · 开放源代码 · 非游戏橘子官方</span>
    <h1>《新枫之谷》的<br>登录启动器</h1>
    <p class="landing-lead">更快捷的账号管理，更好的登录体验。</p>
    <LatestRelease product="maplelink" variant="hero" />
    <p class="landing-fine">只从本站或 GitHub Releases 下载。<a :href="withBase('/zh-CN/download#verify')">核对 SHA256</a> · <a :href="withBase('/zh-CN/guide/')">新手教学</a><br>也玩其他橘子游戏？我们同时维护原版 <a href="https://github.com/pungin/Beanfun">Beanfun</a> 启动器。</p>
  </div>
  <div class="landing-window"><AppWindow /></div>
</section>

<section class="landing-help">
  <div>
    <h2>开始使用</h2>
    <ul>
      <li><a :href="withBase('/zh-CN/guide/')">新手教学</a>：下载、解压、登录、取 OTP，约五分钟</li>
      <li>需要 Windows 10 / 11（64 位）及 WebView2</li>
      <li>不需要安装，整个文件夹放哪里都可以</li>
    </ul>
  </div>
  <div>
    <h2>遇到问题</h2>
    <ul>
      <li><a :href="withBase('/zh-CN/faq#webview2')">打不开、白屏、闪退</a>：安装 WebView2</li>
      <li><a :href="withBase('/zh-CN/faq')">SmartScreen 警告、杀毒误判、加速器认不到</a></li>
      <li>其他问题到 <a href="https://github.com/lshw54/maplelink/issues">GitHub Issues</a> 反馈</li>
    </ul>
  </div>
  <div>
    <h2>下载与更新</h2>
    <ul>
      <li>只从本站或 <a href="https://github.com/lshw54/maplelink/releases">GitHub Releases</a> 下载</li>
      <li>运行前<a :href="withBase('/zh-CN/download#verify')">核对 SHA256</a>，不一致就删除</li>
      <li>已在使用的话，程序会自动更新</li>
    </ul>
  </div>
</section>

<section class="landing-legal">
  <p>MapleLink 与 <a href="https://github.com/pungin/Beanfun">Beanfun</a> 均为第三方开放源代码软件，由同一批人并行维护，与游戏橘子（Gamania）、beanfun! 及 Nexon 没有任何关联，也不是它们的官方产品。使用第三方启动器可能违反游戏服务条款，请自行评估风险；因使用本软件造成的任何账号或财物损失，开发团队不承担责任。</p>
  <p>记住的账号密码只保存在你的电脑，以 Windows DPAPI 加密，不会传送给开发团队。代码以 <a href="https://github.com/lshw54/maplelink/blob/main/LICENSE">MIT 许可</a>公开于 GitHub，任何人都可以查看与核对。</p>
</section>

</div>
