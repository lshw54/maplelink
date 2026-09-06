---
title: Beginner guide
outline: [2, 3]
---

# Beginner guide

From download to in-game in about five minutes. Follow the steps in order.

## 1. Download

Go to the [download page](/en/download). First time, pick **MapleLink-Setup.exe**; see "Which file" on that page for other cases.

::: tip Only download from this site or GitHub Releases
A file from anywhere else did not come from us. After downloading you can [check the SHA256](/en/download#verify).
:::

## 2. Unpack and keep it

Double-click `MapleLink-Setup.exe` and pick a location, for example `D:\Games\`. The unpacked folder holds only the program and a readme. Settings and accounts are not kept there, so where you put the app, and whether you move it later, makes no difference.

::: details Where everything is stored
| Location | Contents |
|----------|----------|
| The folder you unpacked | `MapleLink.exe` itself, plus a readme on the WebView2 error |
| `%APPDATA%\com.maplelink.app\` | `config.ini` settings; `accounts.dat` and `accounts.key`, the encrypted credentials; `lr\`, the Locale Remulator files for locale emulation |
| `%LOCALAPPDATA%\com.maplelink.app\` | `logs\maplelink.log`; `EBWebView\`, the WebView2 cache |

To open one of these, press <kbd>Win</kbd>+<kbd>R</kbd>, paste the path (for example `%APPDATA%\com.maplelink.app`) and press Enter.

Auto-update replaces only `MapleLink.exe` and never touches the two folders above. When reinstalling Windows or moving to a new PC, use "Export backup" in the toolbox; do not copy `accounts.dat` by hand, as it is bound to the original machine.
:::

## 3. First launch

Double-click `MapleLink.exe`, then choose your language and region (HK or TW). The first launch may bring up one of these:

::: details Windows says "unrecognized app"
That is SmartScreen. The app has no commercial signing certificate; this is not a virus warning. After checking the SHA256, click "More info", then "Run anyway". Try it below:

<DemoSmartScreen />
:::

::: details The window is blank, or closes at once
WebView2 is missing. Follow the [FAQ steps](/en/faq#webview2); it takes about a minute.

<DemoBlank />
:::

::: details An announcement appears
Read it and close it. You can reread it from the toolbox later.

<DemoAnnouncement />
:::

::: details The app offers to rename itself to Beanfun.exe
Some game accelerators only recognise that file name. Accepting changes nothing else; skip it if you do not use one.

<DemoRename />
:::

## 4. Sign in

Switch region at the top right of the sign-in page. Below is a working copy of that page; flip the flag and fill in the fields:

<DemoLogin />

### HK

Enter your Beanfun username and password. If two-factor authentication is on, enter the six digits from your authenticator. Image CAPTCHAs are solved by the app; you only type one if that fails.

Tick "Remember password" and the account appears in the list on the left next time. Click it to sign in.

### TW

Pick any of three:

- **QR Code**: scan the code on screen with the beanfun! app on your phone and confirm there.
- **GamaPass**: complete the check in the GamaPass app as prompted.
- **Username and password**: requires a reCAPTCHA, which the app opens in a small window for you.

## 5. Set the game path

Before the first launch, open Toolbox → Settings and point the app at the folder containing `MapleStory.exe`. It tries to detect the path first; choose by hand only if that fails. The toolbox is the 🛠 at the top right of the title bar.

<DemoSettings focus="path" />

## 6. Get an OTP, start the game

After signing in, the right side of the main window lists the game accounts under that Beanfun account. Select one and click "Get OTP":

<DemoAccounts />

- by default it is copied to the clipboard;
- with "Auto-type" on, the app types it into the game's sign-in window.

OTPs expire, so use them at once.

Then press the round button on the left of the main window to start the game:

<DemoPlay />

::: details Just want to start the game and sign in inside it
The sign-in page has "Launch game directly". No account needed.
:::

## Further features

Once the basics feel familiar:

::: details Many accounts and sessions
Click + at the end of the tab strip to sign in to another account, in a different region if you like. Each tab keeps its own account list, OTP and launch buttons. Account cards can be dragged to reorder and right-clicked to rename.

<DemoSessions />
:::

::: details Locale emulation
Automatic, with no switch. When the system locale is not Traditional Chinese, the app launches the game through Locale Remulator by itself; nothing to install or set.
:::

::: details Block auto-update
Under Toolbox → Advanced. When on, `Patcher.exe` is closed as the game starts. Leave it off normally.

<DemoSettings focus="patcher" />
:::

::: details Classic server
Choose "Classic" in the service list. The app checks for Nexon Game Manager; "Launch" completes SSO and opens the game.
:::

::: details Intercept web launch
For when you can only sign in on the website (for example through UU or a VPN). Open it from the globe icon beside the sign-in button, turn on the switch, and from then on pressing Start Game on the beanfun website launches through MapleLink, which pastes the OTP for you.

<DemoWebLaunch />
:::

::: details New PC
Credentials live in `%APPDATA%\com.maplelink.app\accounts.dat`, encrypted with Windows DPAPI and bound to the original machine, so they cannot be copied. Under Toolbox → Account Manager click "Export data", then "Import data" on the new PC.

<DemoBackup />
:::

If something goes wrong, check the [FAQ](/en/faq) first, then open an issue on [GitHub](https://github.com/lshw54/maplelink/issues).
