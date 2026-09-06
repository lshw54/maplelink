---
title: 常见问题
outline: [2, 3]
---

# 常见问题

依情况分类。点开标题下方的区块可以看到做法。

## 安装与打开

### 打不开、白屏、提示 WebView2 错误 {#webview2}

程序需要「Microsoft Edge WebView2 运行时」才能显示界面。Windows 10 / 11 通常已内置；缺少时请手动安装。

<DemoBlank />

::: details 安装步骤
1. 直接下载「常青版引导安装程序」：<https://go.microsoft.com/fwlink/p/?LinkId=2124703>（文件名为 `MicrosoftEdgeWebView2Setup.exe`）。也可以到[微软官方下载页](https://developer.microsoft.com/microsoft-edge/webview2/)获取。
2. 运行安装。
3. 重新打开 MapleLink。

中国大陆用户若下载缓慢，可先开启加速器再下载。
:::

### Windows SmartScreen 说「无法识别的应用」

程序没有购买商业代码签名证书，Windows 对它没有信誉记录。这不是病毒警告。

::: details 做法
先依[下载页](/zh-CN/download#verify)核对 SHA256，一致后点「更多信息」再点「仍要运行」。

<DemoSmartScreen />
:::

### 杀毒软件把它当成病毒

第三方启动器常被误判，因为它会启动另一个程序、注入 DLL（区域模拟）并操作剪贴板。

::: details 做法
核对 SHA256 一致后，把该文件加入杀毒软件的白名单。若 SHA256 不一致，请立即删除。
:::

### 加速器认不到程序

部分加速器只认得 `Beanfun.exe`。

::: details 做法
把 `MapleLink.exe` 改名为 `Beanfun.exe` 即可，功能完全相同。第一次打开时程序也会提示一键改名：

<DemoRename />
:::

## 登录

### 显示 no auth key in response 或 missing akey in response URL

登录失败时会在登录钮上方出现红字「Invalid credentials: login failed: no auth key in response」，或右下角弹出「missing akey in response URL」。两个信息代表同一件事：Beanfun 没有回传登录凭证。原因有两类，信息本身分不出是哪一类。

<DemoLoginError />

::: details 做法
1. **先排除账密错误**：到官网用同一组账密登录一次。HK 账号到 <https://hk.beanfun.com/>，TW 账号到 <https://tw.beanfun.com/>。登录不了就是账密问题，请在官网重设密码。
2. **官网登录得了，就是连接节点问题**：见下一条。
3. 两者都没问题仍然失败，多数是 Beanfun 服务器暂时异常，等几分钟再试。
:::

### 连接节点有问题时会看到什么

使用加速器或 VPN 时，节点不稳或被 Beanfun 拒绝，会有三种表现：登录钮上方出现「failed to extract session key (no OTP1 span)」；官网的进阶验证窗口「感谢您的配合，您的资料已验证成功…」点了确定又再弹出，无限重复；或出现 `Request timeout: https://tw.beanfun.com/…default.aspx?service=999999_T0`。

<DemoNodeErrors />

::: details 做法
1. **换节点**：在加速器或 VPN 换一个节点再试；能直接连接的话，暂时关掉再试。
2. **用加速器的人，确认程序已改名为 `Beanfun.exe`**：加速器是按进程名称加速的，没改名就认不到 MapleLink，流量根本没有经过节点。做法见上方「加速器认不到程序」。
3. 无限重复验证的情况，换节点后请关掉程序重开，再登录一次。
:::

### 图形验证码一直失败

程序会自动处理图形验证码，连续失败时会退回手动输入。

::: details 做法
在弹出的窗口手动输入验证码。若持续失败，多数是 Beanfun 服务器暂时异常，稍后再试。
:::

### TW 账密登录卡在 reCAPTCHA

TW 一般账密登录需要通过 reCAPTCHA。

::: details 做法
程序会打开一个小窗口让你完成验证，完成后自动继续。若窗口没有出现，可以改用 QR Code 或 GamaPass 登录。
:::

## 游戏启动

### 点「开始游戏」没有反应

多数是游戏路径未设置或不正确。

::: details 做法
到工具箱「设置」确认 `MapleStory.exe` 所在的文件夹。程序会先尝试自动检测，检测不到时请手动选择。

<DemoSettings focus="path" />
:::

### 游戏出现乱码或无法启动

系统区域不是繁体中文时会发生。

::: details 做法
程序会自动检测系统区域，不是繁体中文时就经 Locale Remulator 启动游戏，没有开关。仍然乱码的话，请先确认游戏路径正确，再到 [GitHub Issues](https://github.com/lshw54/maplelink/issues) 附上日志反馈。
:::

## 账号与数据

### 更换电脑后账号列表不见了

账号存放在 `%APPDATA%\com.maplelink.app\accounts.dat`，以 Windows DPAPI 加密，绑定原本的电脑与 Windows 账户，无法直接复制。

::: details 做法
在旧电脑的工具箱「账号管理」点「导出数据」，再于新电脑「导入数据」。

<DemoBackup />
:::

### 密码会传到哪里

只存放在你的电脑。记住的账号密码以 Windows DPAPI 加密写入 `%APPDATA%\com.maplelink.app\accounts.dat`，不会传送给开发团队。登录时只与 Beanfun 官方服务器通讯。

## 关于项目

### MapleLink 与 Beanfun 有什么区别

两者由同一批人并行维护。MapleLink 专为《新枫之谷》玩家打造，由零重写，新技术与登录问题的修正会先在这里出现；[Beanfun](https://github.com/pungin/Beanfun) 以支持所有橘子旗下游戏为目标。详见 [pungin/Beanfun#323](https://github.com/pungin/Beanfun/issues/323)。

### 如何反馈问题

到 [GitHub Issues](https://github.com/lshw54/maplelink/issues) 开一则 issue，附上程序版本、地区与复现步骤。
