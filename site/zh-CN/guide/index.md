---
title: 新手教学
outline: [2, 3]
---

# 新手教学

从下载到进入游戏，约五分钟。依序完成即可。

## 1. 下载

到[下载页](/zh-CN/download)。第一次使用请选 **MapleLink-Setup.exe**，其他情况见该页的「选择文件」。

::: tip 只从本站或 GitHub Releases 下载
从其他地方取得的文件都不是我们发出的。下载后可以[核对 SHA256](/zh-CN/download#verify)。
:::

## 2. 解压并放好

双击 `MapleLink-Setup.exe`，选择一个位置解压，例如 `D:\Games\`。解出的文件夹只有程序本身和一份说明；设置与账号不会存在这里，所以程序放在哪里、之后要不要搬，都没有影响。

::: details 各项数据存放在哪里
| 位置 | 内容 |
|------|------|
| 你解压的文件夹 | `MapleLink.exe` 程序本身，加一份「webview2報錯看這.txt」 |
| `%APPDATA%\com.maplelink.app\` | `config.ini` 设置、`accounts.dat` 与 `accounts.key` 加密后的账号密码、`lr\` 区域模拟用的 Locale Remulator 文件 |
| `%LOCALAPPDATA%\com.maplelink.app\` | `logs\maplelink.log` 日志、`EBWebView\` WebView2 缓存 |

要打开这些位置，按 <kbd>Win</kbd>+<kbd>R</kbd>，粘贴路径（例如 `%APPDATA%\com.maplelink.app`）再按 Enter。

自动更新只会替换 `MapleLink.exe`，不会动到上面两个文件夹。重装或更换电脑时，请用工具箱的「导出备份」，不要直接复制 `accounts.dat`，它与原本的电脑绑定。
:::

## 3. 第一次打开

双击 `MapleLink.exe`，然后选择语言与地区（HK 或 TW）。第一次打开可能遇到以下情况：

::: details Windows 显示「无法识别的应用」
这是 SmartScreen 提示，原因是程序没有购买商业签名证书，并非病毒警告。核对过 SHA256 后，点「更多信息」再点「仍要运行」。下面是可以操作的示范：

<DemoSmartScreen />
:::

::: details 窗口全白，或一打开就关闭
电脑缺少 WebView2。依[常见问题的步骤](/zh-CN/faq#webview2)安装，约一分钟。

<DemoBlank />
:::

::: details 出现一段公告
阅读后关闭即可。日后可在工具箱重看。

<DemoAnnouncement />
:::

::: details 程序询问是否改名为 Beanfun.exe
部分网游加速器只认得这个文件名。同意改名不影响任何功能；不使用加速器可以略过。

<DemoRename />
:::

## 4. 登录

在登录页右上角可以切换地区。下面是可以操作的登录页，试试切换旗标、填入账密：

<DemoLogin />

### HK

输入 Beanfun 账号与密码。已启用双重验证的账号，再输入验证器上的六位数字。图形验证码由程序自动处理，失败时才需要手动输入。

勾选「记住密码」后，下次账号会出现在左侧列表，点选即可登录。

### TW

三种方式任选其一：

- **QR Code**：用手机的 beanfun! App 扫描画面上的 QR Code，在手机上确认。
- **GamaPass**：依画面指示在 GamaPass App 完成验证。
- **账号密码**：需要通过 reCAPTCHA，程序会打开一个小窗口让你完成。

## 5. 设置游戏路径

第一次启动游戏前，到工具箱的「设置」指定 `MapleStory.exe` 所在的文件夹。程序会先尝试自动检测，检测不到时才需要手动选择。工具箱在标题栏右上角的 🛠。

<DemoSettings focus="path" />

## 6. 获取 OTP，启动游戏

登录后，主页右侧会列出这个账号下的游戏账号。点选一个，再点「获取 OTP」：

<DemoAccounts />

- 默认会复制到剪贴板；
- 开启「自动输入」后，程序会直接填入游戏登录窗口。

OTP 有时效，获取后请立即使用。

然后点主页左侧的圆形按钮启动游戏：

<DemoPlay />

::: details 只想启动游戏，在游戏内自行登录
登录页有「直接启动游戏」，不需要先登录任何账号。
:::

## 进阶功能

熟悉基本操作后，可以再试以下功能：

::: details 多账号与多 Session
点标签栏右边的「+」可以再登录一个账号，地区不同也可以；每个标签各自有账号列表、OTP 与启动按钮。账号卡片可拖拽排序、右键改名。

<DemoSessions />
:::

::: details 区域模拟
自动进行，没有开关。系统区域不是繁体中文时，程序启动游戏会自动经 Locale Remulator 注入，不需另外安装或设置。
:::

::: details 阻止自动更新
在工具箱「高级」开启后，游戏启动时自动关闭 `Patcher.exe`。平时建议保持关闭。

<DemoSettings focus="patcher" />
:::

::: details 怀旧服
在服务列表选「经典版」。程序会检查 Nexon Game Manager 是否已安装，点「启动」自动完成 SSO 并打开游戏。
:::

::: details 拦截网页启动
只能用网页登录（例如经 UU／VPN）时使用。在登录页点登录钮右边的地球图标进入设置页，开启开关后，在浏览器登录 beanfun 官网并点「开始游戏」，会改由 MapleLink 开游戏并贴入 OTP。

<DemoWebLaunch />
:::

::: details 更换电脑
账号存放在 `%APPDATA%\com.maplelink.app\accounts.dat`，以 Windows DPAPI 加密并绑定原本的电脑，无法直接复制。在工具箱「账号管理」点「导出数据」，再于新电脑「导入数据」。

<DemoBackup />
:::

遇到问题请先查看[常见问题](/zh-CN/faq)；找不到答案，到 [GitHub Issues](https://github.com/lshw54/maplelink/issues) 反馈。
