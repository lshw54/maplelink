<p align="center">
  <img src="public/app-icon.png" width="80" />
</p>

<h1 align="center">MapleLink</h1>

<p align="center">A next-gen third-party Beanfun launcher</p>

<p align="center">
  <a href="https://github.com/lshw54/maplelink/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/lshw54/maplelink/ci.yml?label=CI" alt="CI" /></a>
  <a href="https://github.com/lshw54/maplelink/actions/workflows/build.yml"><img src="https://img.shields.io/github/actions/workflow/status/lshw54/maplelink/build.yml?label=build" alt="Build" /></a>
  <a href="https://github.com/lshw54/maplelink/releases/latest"><img src="https://img.shields.io/github/v/release/lshw54/maplelink?include_prereleases&label=version" alt="Version" /></a>
  <a href="https://github.com/lshw54/maplelink/releases"><img src="https://img.shields.io/github/downloads/lshw54/maplelink/total" alt="Downloads" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License" /></a>
</p>

<p align="center">
  <a href="../../releases/latest">Download</a> · <a href="#features">Features</a> · <a href="#development">Dev Guide</a> · <a href="README.md">繁體中文</a>
</p>

---

⚠️ **This is NOT an official Gamania product.** Use at your own risk. Make sure you trust where you got this from.

## Why MapleLink?

The original [Beanfun launcher](https://github.com/pungin/Beanfun) served well but was showing its age — .NET WinForms, hard to maintain and extend. MapleLink is a ground-up rewrite built for the long run:

- **Rust backend** — all business logic lives in Rust. Session management, OTP, account parsing, DLL injection, process control. No shortcuts.
- **Tauri v2 + WebView2** — lightweight native shell. Small binary, low memory, fast startup.
- **React 19 + Tailwind** — clean, modern frontend with full styling freedom.
- **Clean Architecture** — `commands/` → `core/` → `services/` → `models/`. Structured to stay maintainable as features grow.
- **Single config** — one `config.ini` for both HK and TW regions.

## Features

| Category | Feature | Details |
|----------|---------|---------|
| 🔐 Auth | Account/password | Per-region password saving |
| | TOTP verification | Two-factor auth for HK region |
| | QR Code login | Scan-to-login for TW region |
| | GamePass login | TW region GamePass authentication |
| | Advance Check | Automatic CAPTCHA handling |
| 👥 Accounts | Multi-account | Account list, context menu, drag reorder, custom names |
| | Multi-session | Log into multiple accounts simultaneously, cross-region |
| | Direct launch | Start the game from login page without signing in |
| 🎮 Launch | One-click OTP | Auto-copy or auto-paste into game window |
| | Locale emulation | Auto-inject via [Locale Remulator](https://github.com/InWILL/Locale_Remulator) |
| | Block auto-update | Optionally kill Patcher.exe on launch |
| 🌍 Region | HK + TW | Full support for both regions |
| 🎨 UI | Themes | Dark / Light / System |
| | Languages | English, 繁體中文, 简体中文 |
| | DPI-aware | Unaffected by Windows text size settings |
| 🔄 Update | Auto-update | Via GitHub Releases with proxy detection and fallback |
| | Download progress | Speed, percentage, background download, restart later |
| 🛠 Tools | Debug console | Real-time logs, sensitive data masking, filter/search/copy |
| | Accelerator-friendly | Compatible with UU and other game accelerators, SSL tolerance |

## Getting Started

**Requirements:** Windows 10+, [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (built into Win11)

1. Grab the latest build from [Releases](../../releases/latest)
2. Install and run

> The `EBWebView` folder in `%APPDATA%` is WebView2's cache — this is normal. Enable "GamePass Incognito Mode" in settings if you don't want it saving login sessions.

## Tech Stack

| Layer | Tech |
|-------|------|
| Backend | [Rust](https://www.rust-lang.org/) + [Tauri v2](https://v2.tauri.app/) |
| Frontend | [React 19](https://react.dev/) + TypeScript |
| Styling | [Tailwind CSS v4](https://tailwindcss.com/) |
| State | [Zustand](https://zustand.docs.pmnd.rs/) + [TanStack Query](https://tanstack.com/query) |
| Locale | [Locale Remulator](https://github.com/InWILL/Locale_Remulator) |

<details>
<summary>Architecture</summary>

The Rust backend owns all business logic, side effects, and data. The React/TypeScript frontend is a pure presentation layer that invokes Tauri commands and renders state.

### Design Principles

1. **Rust as single source of truth** — validation, auth, config parsing, DLL injection, process management all in Rust. Frontend does no business logic.
2. **Layered architecture** — `commands/` → `core/` → `services/` → `models/`, following Clean Architecture.
3. **INI config round-trip guarantee** — serialize then parse back = identical values.
4. **In-memory-only credentials** — session tokens and passwords never touch disk. Cleared on exit/logout.
5. **DLL integrity check** — SHA-256 verification before Locale_Remulator injection.

### High-Level Architecture

```mermaid
graph TB
    subgraph Frontend ["Frontend (React 19 + TypeScript)"]
        UI[UI Components]
        Zustand[Zustand Store]
        TQ[TanStack Query]
        Invoker[Typed Tauri Invoker]
    end

    subgraph Backend ["Backend (Rust / Tauri v2)"]
        Commands[commands/]
        Core[core/]
        Services[services/]
        Models[models/]
    end

    subgraph External ["External"]
        Beanfun[Beanfun API]
        FS[File System]
        Process[OS Processes]
        Updater[GitHub Releases]
    end

    UI --> Zustand
    UI --> TQ
    TQ --> Invoker
    Zustand --> Invoker
    Invoker -->|IPC| Commands
    Commands --> Core
    Commands --> Services
    Core --> Models
    Services --> Models
    Services --> Beanfun
    Services --> FS
    Services --> Process
    Services --> Updater
```

### Request Flow

```mermaid
sequenceDiagram
    participant FE as Frontend
    participant CMD as commands/
    participant CORE as core/
    participant SVC as services/
    participant EXT as External

    FE->>CMD: invoke("login", {account, password})
    CMD->>CMD: Validate & deserialize args
    CMD->>CORE: auth::authenticate(credentials)
    CORE->>SVC: BeanfunService::login(credentials)
    SVC->>EXT: HTTPS POST to Beanfun
    EXT-->>SVC: Response
    SVC-->>CORE: Session | Error
    CORE-->>CMD: Result<Session, AuthError>
    CMD->>CMD: Map to serializable response
    CMD-->>FE: Ok(SessionDto) | Err(ErrorDto)
```

### Project Structure

```
src-tauri/src/
├── commands/
│   ├── auth.rs                # login, logout, QR, TOTP, GamePass, session management
│   ├── account.rs             # game accounts, OTP retrieval, refresh
│   ├── launcher.rs            # launch game, direct launch, process status
│   ├── config.rs              # config read/write/reset
│   ├── update.rs              # update check, streaming download, restart
│   └── system.rs              # file dialog, version, logging, popup windows
├── core/                      # Pure business logic (auth, config parser, DLL injector, error)
├── services/                  # Side effects (HTTP, file I/O, process management, updates, proxy detection)
├── models/                    # DTOs and domain structs (incl. SessionState for multi-session)
└── utils/                     # Helpers (SHA-256, etc.)

src/
├── features/
│   ├── login/                 # Login page (normal, QR, TOTP, verify)
│   ├── launcher/              # Main page (account grid, OTP, launch)
│   ├── toolbox/               # Toolbox (settings, account manager, about)
│   └── shared/                # Titlebar, status bar, error toast
├── lib/                       # Tauri invoker, i18n, Zustand stores, hooks
├── styles/                    # Tailwind + CSS variables
└── locales/                   # en-US / zh-TW / zh-CN
```

### Pages & Window Sizes

| Page | Size (logical) | Description |
|------|---------------|-------------|
| Login | 350 × 620 | Login forms |
| Main | 760 × 530 | Account grid, OTP, launch button |
| Toolbox | 750 × 490 | Tools, settings, account manager, about |

</details>

## Development

```bash
npm install                # install frontend deps
cargo tauri dev            # dev mode (hot reload)
cargo tauri build          # production build
```

### Code Standards

```bash
# Rust
cargo fmt --all --check                    # format check
cargo clippy --all-targets -- -D warnings  # lint
cargo test                                 # unit + property tests

# TypeScript
npm run lint                                       # ESLint
npx prettier --check "src/**/*.{ts,tsx,css,json}"  # format check
npx tsc -b                                         # type check
npm run format                                     # Prettier format

# Git commits follow Conventional Commits
# feat: / fix: / refactor: / chore: ...
```

## Contributing

Fork → branch → test → PR.

## Credits

Inspired by [pungin/Beanfun](https://github.com/pungin/Beanfun).

## License

MIT
