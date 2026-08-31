import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { commands } from "../../lib/tauri";
import { getTranslation } from "../../lib/i18n";
import { applyAccent } from "../../lib/accent";
import type { Language } from "../../lib/stores/ui-store";
import { ConnectionPanel } from "./ConnectionPanel";
import type { BrowserBookmark, BrowserConnectionInfo, BrowserNavEvent } from "../../lib/types";

/** Matches `BAR_HEIGHT` in `services/browser_window.rs`. */
const BAR_HEIGHT = 46;

/** How tall the toolbar grows while the padlock panel is open. */
const PANEL_HEIGHT = 300;

const BUTTON =
  "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-[15px] text-[var(--text)] transition-colors hover:bg-[var(--surface-hover)] disabled:pointer-events-none disabled:opacity-25";

/** Theme and language come from the app config; this webview has no store. */
function useChromeConfig() {
  const [language, setLanguage] = useState<Language>("zh-TW");

  useEffect(() => {
    let cancelled = false;

    commands
      .getConfig()
      .then((config) => {
        if (cancelled) return;
        setLanguage(config.language);
        applyAccent(config.accentColor);

        const root = document.documentElement;
        const light =
          config.theme === "light" ||
          (config.theme === "system" && window.matchMedia("(prefers-color-scheme: light)").matches);
        root.classList.toggle("light", light);
      })
      .catch(() => {
        /* falls back to the defaults above */
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return language;
}

/**
 * The Beanfun browser's toolbar — a child webview pinned above the content view.
 *
 * It holds no navigation state of its own: every value here arrives from
 * WebView2 through the `browser:nav` event, so the arrows and the address bar
 * describe the page that is actually loaded rather than the one we last asked
 * for. See `services/browser_window.rs`.
 */
export function BrowserChrome() {
  const language = useChromeConfig();
  const t = useCallback((key: string) => getTranslation(language, key), [language]);

  const [nav, setNav] = useState<BrowserNavEvent>({
    url: "",
    title: "",
    canGoBack: false,
    canGoForward: false,
    loading: false,
    partial: false,
  });
  const [draft, setDraft] = useState<string | null>(null);
  const [bookmarks, setBookmarks] = useState<BrowserBookmark[]>([]);
  /** Translation key for something the backend refused to do, shown briefly. */
  const [notice, setNotice] = useState<string | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  const [connection, setConnection] = useState<BrowserConnectionInfo | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // First, before anything that can throw. `listen` reaches into
    // `__TAURI_INTERNALS__` and throws synchronously when it is absent, which
    // would take the rest of this effect — the ping included — down with it,
    // and a toolbar that cannot report in looks exactly like one that never
    // loaded.
    commands.browserToolbarReady().catch(() => {});

    const unlisten = listen<BrowserNavEvent>("browser:nav", (event) => {
      const next = event.payload;
      setNav((prev) =>
        // A partial event knows the URL and nothing else; keeping the previous
        // history flags stops the arrows blinking off between the two emits.
        next.partial ? { ...prev, url: next.url, loading: next.loading, partial: true } : next,
      );
      // Whatever the user was typing is stale once the page moves on its own.
      setDraft(null);
    });

    commands
      .browserState()
      .then((state) => setNav({ ...state, loading: false, partial: false }))
      .catch(() => {
        /* the content view may not have finished its first navigation yet */
      });
    commands
      .browserBookmarks()
      .then(setBookmarks)
      .catch(() => setBookmarks([]));

    return () => {
      unlisten.then(
        (off) => off(),
        () => {},
      );
    };
  }, []);

  // A refused navigation or download is invisible otherwise: the click just
  // does nothing, which reads as a broken window rather than a deliberate one.
  useEffect(() => {
    const unlisten = listen<string>("browser:notice", (event) => setNotice(event.payload));
    return () => {
      unlisten.then(
        (off) => off(),
        () => {},
      );
    };
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), 5000);
    return () => clearTimeout(timer);
  }, [notice]);

  /** Close the panel whenever the page underneath changes out from under it. */
  useEffect(() => {
    if (panelOpen) closePanel();
    // Only the address matters here; re-running on every loading flip would
    // shut the panel the moment it opened.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nav.url]);

  function openPanel() {
    setConnection(null);
    setPanelOpen(true);
    commands.browserSetChromeHeight(PANEL_HEIGHT).catch(() => {});
    commands
      .browserConnectionInfo()
      .then(setConnection)
      .catch(() => setConnection(null));
  }

  function closePanel() {
    setPanelOpen(false);
    setConnection(null);
    commands.browserSetChromeHeight(BAR_HEIGHT).catch(() => {});
  }

  function go(url: string) {
    commands.browserNavigate(url).catch(() => {
      /* an unusable address just leaves the page where it is */
    });
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") {
      const value = (draft ?? nav.url).trim();
      if (value) go(value);
      inputRef.current?.blur();
    }
    if (e.key === "Escape") {
      setDraft(null);
      inputRef.current?.blur();
    }
  }

  const secure = nav.url.startsWith("https://");

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <div
        className="relative flex shrink-0 items-center gap-1 border-b border-[var(--tb-border)] bg-[var(--tb-nav-bg)] px-2"
        style={{ height: BAR_HEIGHT }}
      >
        <button
          className={BUTTON}
          disabled={!nav.canGoBack}
          title={t("browser.back")}
          onClick={() => commands.browserBack().catch(() => {})}
        >
          ‹
        </button>
        <button
          className={BUTTON}
          disabled={!nav.canGoForward}
          title={t("browser.forward")}
          onClick={() => commands.browserForward().catch(() => {})}
        >
          ›
        </button>
        <button
          className={BUTTON}
          title={t("browser.reload")}
          onClick={() => commands.browserReload().catch(() => {})}
        >
          ⟳
        </button>

        {/* The padlock. Its state comes from the address alone; what the panel
          shows behind it takes a handshake, so it is fetched on demand. */}
        <button
          className={BUTTON}
          title={t("browser.connection.title")}
          onClick={() => (panelOpen ? closePanel() : openPanel())}
        >
          {secure ? "🔒" : "⚠"}
        </button>

        <input
          ref={inputRef}
          value={draft ?? nav.url}
          spellCheck={false}
          placeholder={t("browser.address_placeholder")}
          onChange={(e) => setDraft(e.target.value)}
          onFocus={(e) => e.currentTarget.select()}
          onBlur={() => setDraft(null)}
          onKeyDown={onKeyDown}
          className="mx-1 h-8 min-w-0 flex-1 rounded-lg border border-[var(--tb-border)] bg-[var(--tb-input-bg)] px-3 text-[12px] text-[var(--text)] outline-none placeholder:text-[var(--text-faint)] focus:border-[rgba(var(--accent-rgb),0.5)]"
        />

        {/* A native select, not a styled menu. This webview is only as tall as the
          bar, and WebView2 renders nothing outside its own bounds — a menu
          positioned under the button is drawn where no pixels exist. The
          browser process opens a select's list as an OS-level popup, which is
          not bound by any of that. */}
        <select
          title={t("browser.bookmarks")}
          value=""
          onChange={(e) => {
            const url = e.target.value;
            e.target.value = "";
            if (url) go(url);
          }}
          className="h-8 shrink-0 rounded-lg border border-[var(--tb-border)] bg-[var(--tb-input-bg)] px-2 text-[12px] text-[var(--text)] outline-none"
        >
          <option value="">{t("browser.bookmarks")}</option>
          {bookmarks.map((mark) => (
            <option key={mark.key} value={mark.url}>
              {t(mark.key)}
            </option>
          ))}
        </select>

        <button
          className={BUTTON}
          title={t("browser.open_external_hint")}
          onClick={() => {
            if (nav.url) commands.browserOpenExternal(nav.url).catch(() => {});
          }}
        >
          ↗
        </button>

        {notice && (
          <div className="absolute inset-x-0 bottom-0 flex justify-center">
            <div className="mb-1 max-w-[90%] truncate rounded-md bg-[var(--tb-card)] px-3 py-1 text-[11px] text-[var(--text-dim)] shadow-[0_2px_10px_rgba(0,0,0,0.35)]">
              {t(notice)}
            </div>
          </div>
        )}

        {nav.loading && (
          <div className="absolute inset-x-0 bottom-0 h-[2px] overflow-hidden">
            <div className="h-full w-1/3 animate-[browserLoad_1.1s_ease-in-out_infinite] bg-accent" />
          </div>
        )}
      </div>

      {panelOpen && <ConnectionPanel info={connection} t={t} onClose={closePanel} />}
    </div>
  );
}
