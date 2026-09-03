import { useMutation, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { getTranslation } from "../i18n";
import { commands, solveRecaptcha } from "../tauri";
import { useAuthStore } from "../stores/auth-store";
import { useConfigStore } from "../stores/config-store";
import { useUiStore } from "../stores/ui-store";
import { useErrorToastStore } from "../stores/error-toast-store";
import type { SessionDto } from "../types";

/** Translate outside React render (mutation callbacks) using the current language. */
const tr = (key: string) => getTranslation(useUiStore.getState().language, key);

/** Launch the game after login if the user enabled autoLaunchGame. Shared by every login path (password, QR, TOTP). */
function autoLaunchGameIfEnabled(sessionId: string) {
  const cfg = useConfigStore.getState().config;
  if (!cfg?.autoLaunchGame) return;
  void (async () => {
    try {
      let pid = 0;
      if (cfg.traditionalLogin) {
        // Direct launch passes no account or OTP, so the account list is irrelevant.
        pid = await commands.launchGameDirect();
      } else {
        // Every login path awaits the account fetch and lands on the account list
        // before asking for a launch, so the store already holds the final list —
        // there is nothing to wait out. An empty list is a legitimate result (a
        // beanfun account with no game account yet); there is just nothing to launch.
        const first = useAuthStore.getState().sessions.get(sessionId)?.gameAccounts?.[0];
        if (!first) return;
        pid = await commands.launchGame(sessionId, first.id);
      }
      if (pid > 0) {
        useUiStore.getState().setGamePid(pid);
        useUiStore.getState().setGameRunning(true);
      }
    } catch {
      /* auto-launch failure is non-critical */
    }
  })();
}

/**
 * Everything that happens once a login of any kind has produced a session:
 * register it, load its game accounts (retrying once if the list came back
 * empty, and saying so if it still is), land on the account list, then either
 * launch Classic on this session or run the regular auto-launch.
 */
export async function finishLogin(queryClient: QueryClient, session: SessionDto) {
  // Classic (懷舊服) and the regular server share the same beanfun account, so
  // a classic login is set up EXACTLY like a regular one — the session joins
  // the account list and gets the same polling / keep-alive — and THEN the
  // classic portal launch is fired below. Either server can then be played
  // from this single login.
  const classic = useUiStore.getState().classicMode;

  useAuthStore.getState().addSession(session);
  let accountCount = -1;
  try {
    let accounts = await commands.getGameAccounts(session.sessionId);
    if (accounts.length === 0) {
      // Login-time list came back empty — force one fresh fetch so the user
      // isn't stranded on an empty account list.
      try {
        accounts = await commands.refreshAccounts(session.sessionId);
      } catch {
        /* keep the empty list; refresh is best-effort */
      }
    }
    accountCount = accounts.length;
    useAuthStore.getState().updateGameAccounts(session.sessionId, accounts);
  } catch {
    /* accounts fetch failure is non-critical */
  }
  // Make an empty account list VISIBLE instead of a silent empty page, so a
  // tester notices + reports it (and can retry) rather than it looking broken.
  if (accountCount === 0) {
    useErrorToastStore.getState().addToast({
      message: tr("login.accounts_load_failed"),
      category: "authentication",
      critical: false,
    });
  }
  await queryClient.invalidateQueries({ queryKey: ["gameAccounts"] });
  // Leave the login page clean for the next session: no "adding" flag, the
  // normal view, and no QR code held over from a scan that just completed.
  useUiStore.setState({
    addingSession: false,
    loginView: "normal",
    qrSessionId: null,
    qrData: null,
    qrIssuedAt: null,
  });
  useUiStore.getState().setPage("main");

  // Classic: the regular session + account list are in place, so launch the
  // classic portal on this same session; the app-level overlay shows its
  // progress over the account list. This replaces the regular auto-launch —
  // the classic launch is this login's launch.
  if (classic) {
    useUiStore.setState({ classicStatus: "launching" });
    commands.openClassicLogin(session.sessionId).catch(() => {
      useUiStore.setState({ classicStatus: "failed" });
    });
    return;
  }

  autoLaunchGameIfEnabled(session.sessionId);
}

/** Login with account + password. Creates a new session, then authenticates. */
export function useLogin() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      account,
      password,
      rememberPassword,
      resumeSessionId,
    }: {
      account: string;
      password: string;
      rememberPassword: boolean;
      /**
       * Set when retrying after a passed advance check: reuse this session
       * (which already holds the verified context + the pending skey/form-token)
       * and only redo the login-step reCAPTCHA — do NOT create a new session or
       * redo CheckAccountType, which would reset beanfun's context and re-trigger
       * advance check in a loop.
       */
      resumeSessionId?: string;
    }) => {
      const region = useConfigStore.getState().config?.region ?? "TW";
      // Resume (advance-check retry) only applies to the TW two-phase flow.
      const resuming = resumeSessionId !== undefined && region === "TW";
      const sessionId =
        resuming && resumeSessionId ? resumeSessionId : await commands.createSession();

      useAuthStore.getState().setPendingCredentials({
        account,
        password,
        rememberPassword,
        sessionId,
      });

      try {
        // TW Regular (帳密) login runs over the two-phase commands
        // (twLoginCheck = CheckAccountType, twLoginSubmit = AccountLogin). We try
        // each step WITHOUT a reCAPTCHA first, so IsRecaptcha=false accounts get
        // the exact v0.3.6 experience (empty tokens, NO popup); only when beanfun
        // replies RecaptchaRequired do we open the on-origin popup (token is
        // domain-locked to login.beanfun.com).
        //
        // IMPORTANT: the non-resume attempt MUST go through twLoginCheck — it
        // stashes the skey/form-token on the session. When AdvanceCheckRequired
        // drives the native VerifyForm and the user passes it, the resume reuses
        // that SAME verified session via twLoginSubmit. (A single-shot `login`
        // here would fetch a fresh skey on resume, dropping beanfun's
        // advance-check verification → login never reaches the account list.)
        let session: SessionDto;
        if (region === "TW") {
          const needsRecaptcha = (e: unknown) => {
            const s = typeof e === "object" && e !== null ? JSON.stringify(e) : String(e);
            return s.includes("RecaptchaRequired") || s.includes("RECAPTCHA_REQUIRED");
          };
          if (!resuming) {
            try {
              await commands.twLoginCheck(sessionId, account, "");
            } catch (e) {
              if (!needsRecaptcha(e)) throw e;
              await commands.twLoginCheck(sessionId, account, await solveRecaptcha("check"));
            }
          }
          try {
            session = await commands.twLoginSubmit(sessionId, password, "");
          } catch (e) {
            if (!needsRecaptcha(e)) throw e;
            session = await commands.twLoginSubmit(
              sessionId,
              password,
              await solveRecaptcha("login"),
            );
          }
        } else {
          session = await commands.login(sessionId, account, password);
        }
        try {
          await commands.saveLoginCredentials(account, password, rememberPassword);
        } catch {
          /* credential save failure is non-critical */
        }
        useAuthStore.getState().setPendingCredentials(null);
        return session;
      } catch (err: unknown) {
        const errStr = typeof err === "object" && err !== null ? JSON.stringify(err) : String(err);
        if (
          errStr.includes("RECAPTCHA_CANCELLED") ||
          errStr.includes("RECAPTCHA_TIMEOUT") ||
          errStr.includes("WEBLOGIN_CANCELLED") ||
          errStr.includes("WEBLOGIN_TIMEOUT")
        ) {
          // User closed the login/verification window, or it never completed —
          // treat as a quiet abort so the button doesn't hang on "登入中...".
          useAuthStore.getState().setPendingCredentials(null);
          throw new Error(tr("login.cancelled"), { cause: err });
        }
        if (errStr.includes("TOTP") || errStr.includes("totp") || errStr.includes("Totp")) {
          const totpError = new Error("TOTP_REQUIRED") as Error & { sessionId: string };
          totpError.name = "TotpRequired";
          totpError.sessionId = sessionId;
          throw totpError;
        }
        if (errStr.includes("ADVANCE_CHECK") || errStr.includes("advance_check")) {
          // B: cap the loop. If advance check is required *again* right after we
          // already passed one (this is a resume), stop instead of re-looping —
          // repeated attempts are what trips beanfun's IP lock.
          if (resuming) {
            useAuthStore.getState().setPendingCredentials(null);
            throw new Error(tr("login.advance_check_repeat"), { cause: err });
          }
          const errObj =
            typeof err === "object" && err !== null ? (err as Record<string, unknown>) : null;
          const url = errObj?.message ? String(errObj.message) : undefined;
          const advError = new Error("ADVANCE_CHECK") as Error & {
            advanceUrl?: string;
            sessionId: string;
          };
          advError.name = "AdvanceCheck";
          advError.advanceUrl = url || undefined;
          advError.sessionId = sessionId;
          throw advError;
        }
        // Login failed — clean up the session
        useAuthStore.getState().setPendingCredentials(null);
        throw new Error(
          typeof err === "object" && err !== null && "message" in err
            ? String((err as Record<string, unknown>).message)
            : String(err),
          { cause: err },
        );
      }
    },
    onSuccess: (session: SessionDto) => finishLogin(queryClient, session),
  });
}

/** Verify TOTP code (HK region). Updates auth store on success. */
export function useTotpVerify() {
  const queryClient = useQueryClient();

  return useMutation<SessionDto, Error, { sessionId: string; code: string }>({
    mutationFn: ({ sessionId, code }) => commands.totpVerify(sessionId, code),
    onSuccess: async (session: SessionDto) => {
      // Save pending credentials from the login attempt
      const pending = useAuthStore.getState().pendingCredentials;
      if (pending) {
        try {
          await commands.saveLoginCredentials(
            pending.account,
            pending.password,
            pending.rememberPassword,
          );
        } catch {
          /* credential save failure is non-critical */
        }
        useAuthStore.getState().setPendingCredentials(null);
      }
      await finishLogin(queryClient, session);
    },
  });
}

/** Logout the active session. Clears it from auth store. */
export function useLogout() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => {
      const sessionId = useAuthStore.getState().activeSessionId;
      if (sessionId) {
        await commands.logout(sessionId);
      }
    },
    onSuccess: async () => {
      const sessionId = useAuthStore.getState().activeSessionId;
      if (sessionId) {
        useAuthStore.getState().removeSession(sessionId);
      }
      queryClient.removeQueries({ queryKey: ["gameAccounts"] });
      // If no sessions left, go to login
      if (useAuthStore.getState().sessions.size === 0) {
        useUiStore.getState().setPage("login");
      }
    },
  });
}
