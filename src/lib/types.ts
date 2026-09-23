/** Mirrors of Rust models for type-safe IPC. */

export interface SessionDto {
  sessionId: string;
  token: string;
  region: "TW" | "HK";
  accountName: string;
  expiresAt: string;
}

export interface GameAccountDto {
  id: string;
  displayName: string;
  gameType: string;
  sn: string;
  status: string;
  createdAt: string;
}

export interface AddServiceAccountDto {
  success: boolean;
  /** beanfun's own reason when it refused, empty otherwise. */
  message: string;
}

export interface AccountLimitDto {
  /** The notice text as beanfun showed it ("" when there was none). */
  notice: string;
  /** Cap on game accounts, when the notice states one. */
  limit: number | null;
  /** TW: the beanfun account must pass advanced verification first. */
  needsVerify: boolean;
}

export interface GameCredentialsDto {
  accountId: string;
  otp: string;
  retrievedAt: string;
}

export interface GameDownloadDto {
  id: number;
  name: string;
  size: string;
  url: string;
  kind: "game" | "patch" | "other";
  /** The Gamania Games Manager installer, which the UI flags rather than recommends. */
  manager: boolean;
}

/** The official manifest, as the client manager window shows it. */
export interface ClientManifestDto {
  productName: string;
  version: string;
  publishDate: string;
  /** When the current executable build was published. */
  exePatchDate: string | null;
  /** Version including the minor part (`"V282.2"`), read off beanfun's own
   *  download page; `version` only ever carries the major. */
  fullVersion: string | null;
  totalBytes: number;
  fileCount: number;
  exeName: string;
  /** When beanfun could not be reached, the time this copy was cached (RFC 3339). */
  cachedAt: string | null;
  /** The manifest the scan compares against, so a player can open it themselves. */
  manifestUrl: string;
}

/** What the installed client's own Base.wz says about its version. */
export interface ClientLocalVersionDto {
  marker: number;
  matchesOfficial: boolean;
  candidates: number[];
}

export type ClientIssueKind =
  | "missing"
  | "sizeMismatch"
  | "hashMismatch"
  | "unreadable"
  | "outdated";

/** One file the scan looked at. `kind` is null when it matches the manifest. */
export interface ClientCheckedFileDto {
  path: string;
  kind: ClientIssueKind | null;
  expectedSize: number;
  localSize: number | null;
}

export interface ClientScanReportDto {
  totalFiles: number;
  okFiles: number;
  files: ClientCheckedFileDto[];
  issueCount: number;
  bytesToFetch: number;
  extraFiles: string[];
  cancelled: boolean;
}

/** Progress for a running scan or download. */
export interface ClientProgressDto {
  done: number;
  total: number;
  bytesDone: number;
  bytesTotal: number;
  current: string;
  /** The files in flight, with how far each has got. Empty during a scan. */
  active: ClientActiveFileDto[];
}

/** Where the official file list was asked for. */
export type ClientManifestSource = "beanfun" | "catalog";

/** One try at one source, sent as it starts. */
export interface ClientManifestAttemptDto {
  source: ClientManifestSource;
  attempt: number;
  attempts: number;
}

/** One list source, asked once. */
export interface ClientSourceTestDto {
  source: ClientManifestSource;
  ok: boolean;
  millis: number;
  version: string | null;
  error: string | null;
}

export interface ClientSourceTestsDto {
  results: ClientSourceTestDto[];
  /** Which source automatic mode asks first after the test. */
  first: ClientManifestSource;
}

/** How the downloads reach the CDN from this machine. */
export interface ClientNetworkStatusDto {
  /** Country code the route comes out in; the address itself is never sent. */
  country: string | null;
  /** The Windows system proxy the downloads go through, if any. */
  proxy: string | null;
  /** Windows has a PAC script set, which is not followed. */
  pac: boolean;
  latencyMs: number | null;
  error: string | null;
}

export interface ClientActiveFileDto {
  path: string;
  done: number;
  total: number;
}

/** How one file is doing during a download, as the backend reports it. */
export type ClientFileState = "downloading" | "done" | "failed";

export interface ClientDownloadFileDto {
  path: string;
  state: ClientFileState;
  error: string | null;
}

export interface ClientDownloadReportDto {
  requested: number;
  written: number;
  failures: { path: string; error: string }[];
  cancelled: boolean;
}

/** What moving extra files to the Recycle Bin did. */
export interface ClientRemoveReportDto {
  removed: string[];
  failures: { path: string; error: string }[];
  bytes: number;
}

/** Full-client torrent details, read from the Gamania Games Manager's public download chain. */
export interface FullClientInfoDto {
  productName: string;
  version: string;
  publishDate: string;
  sizeBytes: number;
  fileCount: number;
  torrentUrl: string;
  folderName: string;
  exeName: string;
  /** The manifest the scan compares against, so a player can open it themselves. */
  manifestUrl: string;
}

/** One selectable game account from GamaPass's classic sign-in chooser. */
export interface ClassicAccountDto {
  value: string;
  label: string;
}

export interface ClassicCheckDto {
  ngmRegistered: boolean;
  ngmExe: string | null;
  ngmExeExists: boolean;
  webview2Version: string | null;
  gameExe: string | null;
}

export interface AppConfigDto {
  gamePath: string;
  theme: "system" | "dark" | "light";
  language: "en-US" | "zh-TW" | "zh-CN";
  autoUpdate: boolean;
  skipPlayConfirm: boolean;
  autoStart: boolean;
  region: "TW" | "HK";
  debugLogging: boolean;
  gamepassIncognito: boolean;
  updateChannel: "release" | "pre-release";
  fontSize: "small" | "medium" | "large" | "extra-large";
  traditionalLogin: boolean;
  autoKillPatcher: boolean;
  accountViewMode: "card" | "list";
  autoLogin: boolean;
  autoLaunchGame: boolean;
  webLaunchAutoLaunch: boolean;
  webLaunchAutoPaste: boolean;
  webLaunchKeepOn: boolean;
  closeBehavior: "ask" | "quit" | "tray";
  hideAccountNames: boolean;
  beanfunRenameDismissed: boolean;
  cafeMode: boolean;
  classicNgmPath: string;
  announcementDismissedId: string;
  webviewViaProxy: boolean;
  otpAutoInput: boolean;
  /** What Enter does on a selected game account; "ask" until chosen. */
  enterAction: "ask" | "copy" | "otp";
  defaultLoginView: "normal" | "qr";
  githubHosts: boolean;
  compactUi: boolean;
  accentColor: string;
}

/** A shortcut in the Beanfun browser's toolbar menu. `key` is a translation key. */
export interface BrowserBookmark {
  key: string;
  url: string;
}

/** Where the Beanfun browser's content view is, and where it can go. */
export interface BrowserNavState {
  url: string;
  title: string;
  canGoBack: boolean;
  canGoForward: boolean;
}

/** The leaf certificate a host is serving, reduced to what is worth reading. */
interface BrowserCertificateInfo {
  subject: string;
  issuer: string;
  /** RFC 2822 — `Date.parse` accepts it. */
  validFrom: string;
  validTo: string;
  fingerprint: string;
  serial: string;
}

/** What the Beanfun browser's padlock shows. */
export interface BrowserConnectionInfo {
  host: string;
  port: number;
  encrypted: boolean;
  /** Null when the handshake failed; `error` then says why. */
  certificate: BrowserCertificateInfo | null;
  error: string | null;
}

/** A `browser:nav` event — the nav state plus how the toolbar should draw it. */
export interface BrowserNavEvent extends BrowserNavState {
  loading: boolean;
  /**
   * True while only the URL is known. The history flags arrive a moment later,
   * so the toolbar keeps its previous ones rather than blinking the arrows off.
   */
  partial: boolean;
}

/** Result of the startup "rename exe to Beanfun.exe" check (China-IP users). */
export interface BeanfunRenameCheck {
  suggest: boolean;
  collision: boolean;
  currentName: string;
  targetName: string;
}

export interface ErrorDto {
  code: string;
  message: string;
  category: "authentication" | "network" | "filesystem" | "process" | "configuration" | "update";
  details?: string;
}

export interface UpdateInfoDto {
  version: string;
  changelog: string;
  downloadUrl: string;
  isPrerelease: boolean;
}

export interface QrCodeData {
  sessionKey: string;
  qrImageUrl: string;
  verificationToken: string;
  deeplink: string;
}

export interface QrPollResult {
  status: "pending" | "scanned" | "confirmed" | "expired";
  session?: SessionDto;
}

export interface SavedAccountDto {
  account: string;
  region: string;
  hasPassword: boolean;
  rememberPassword: boolean;
}

export interface LastSavedAccountDto {
  account: string;
  password: string;
  rememberPassword: boolean;
  verifyInfo?: string | null;
}

export interface WebLaunchStatus {
  registered: boolean;
  gamePath: string;
  gamePathOk: boolean;
  lrReady: boolean;
  gamaniaInstalled: boolean;
  exeName: string;
  exeNameOk: boolean;
}

/** Stable codes returned by the live launch tests, mapped to i18n in the UI. */
export type WebLaunchTestCode =
  | "ok"
  | "skipped_running"
  | "no_game_path"
  | "spawn_failed"
  | "not_found";

export interface AdvanceCheckState {
  viewstate: string;
  viewstateGenerator: string;
  eventValidation: string;
  samplecaptcha: string;
  submitUrl: string;
  captchaImageBase64: string;
  authHint: string;
}
