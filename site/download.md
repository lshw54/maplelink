---
title: 下載
outline: [2, 3]
---

# 下載

<LatestRelease product="maplelink" />

## 選擇檔案

| | 適合誰 | 做法 |
|---|---|---|
| **MapleLink-Setup.exe** | 第一次使用 | 雙擊解壓，得到一個資料夾，內有程式和說明 |
| **MapleLink.exe** | 已在使用、或想自己放位置 | 直接執行。自動更新換的就是這個檔案 |

兩個都不需要安裝，放在任何資料夾都能跑。設定與帳號存放在 `%APPDATA%\com.maplelink.app`，不在程式所在的資料夾，詳見[新手教學](/guide/#_2-解壓並放好)。

## 系統需求

- Windows 10 或 11（64 位元）
- Microsoft Edge WebView2。通常已內建，[缺少時這樣裝](/faq#webview2)
- 懷舊服另需 Nexon Game Manager，程式會自動偵測並提示

## 確認檔案是真的 {#verify}

有人在派發重新打包的啟動器。認可來源只有兩個，其他地方拿到的都不是我們發出的：

1. 本站的下載按鈕
2. <https://github.com/lshw54/maplelink/releases>

本站不存放任何執行檔，按鈕直接連到 GitHub。MapleLink 與 Beanfun 各自獨立發佈，不會互相代發檔案。

### 核對 SHA256

每個版本的發佈說明和上方卡片都列出 `MapleLink.exe` 的 SHA256。

::: details 怎樣核對
1. 若下載的是 `MapleLink-Setup.exe`，先雙擊解壓。
2. 在解出來的資料夾按右鍵，選「在終端機中開啟」，執行：

```powershell
Get-FileHash .\MapleLink.exe -Algorithm SHA256
```

3. 把輸出的 `Hash` 與發佈說明上的值比對。**不一致就刪除，不要執行。**
:::

### 核對簽章（進階）

每次發佈附有 `MapleLink.exe.sig`，由開發團隊的 minisign 私鑰簽署。

::: details 怎樣驗證
公鑰（key ID `64B3B19BBF3B13D1`）：

```
RWTREzu/m7GzZL+ob+fnHI6ktK+56Tw+gqQRv7hWCbSb/+z47yo5EBAy
```

這條公鑰同時編譯在程式裏，可以到 [update_signature.rs](https://github.com/lshw54/maplelink/blob/main/src-tauri/src/services/update_signature.rs) 對照，兩處一致才算數。除非私鑰遺失或外洩，它不會更換。

GitHub 上的 `MapleLink.exe.sig` 是 Tauri 格式，即 minisign 簽章檔再經 base64 編碼，驗證前要先解碼。到 [minisign 的 GitHub Releases](https://github.com/jedisct1/minisign/releases) 下載 Windows 版，解壓得到 `minisign.exe`，不需安裝。把它放到與 `MapleLink.exe`、`MapleLink.exe.sig` 相同的資料夾，然後在該資料夾執行：

```powershell
[IO.File]::WriteAllBytes("MapleLink.exe.minisig", [Convert]::FromBase64String((Get-Content MapleLink.exe.sig -Raw).Trim()))
minisign -Vm MapleLink.exe -x MapleLink.exe.minisig -P RWTREzu/m7GzZL+ob+fnHI6ktK+56Tw+gqQRv7hWCbSb/+z47yo5EBAy
```

顯示 `Signature and comment signature verified` 代表檔案確實由持有私鑰的開發團隊發出，且內容未被更改。
:::

### 為什麼 Windows 仍然警告

::: details 展開
SHA256 與簽章證明「檔案沒有被改過」；Windows SmartScreen 看的是「Windows 是否認識這個發行者」。程式沒有購買商業憑證，所以 SmartScreen 沒有它的信譽紀錄。核對過 SHA256 後，按「其他資訊」再按「仍要執行」即可。

<DemoSmartScreen />
:::

## 所有版本

歷史版本與預覽版在 [GitHub Releases](https://github.com/lshw54/maplelink/releases)。
