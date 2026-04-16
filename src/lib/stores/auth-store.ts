import { create } from "zustand";
import type { GameAccountDto, SessionDto } from "../types";

/** Per-session state stored in the multi-session map. */
export interface SessionEntry {
  sessionId: string;
  session: SessionDto;
  gameAccounts: GameAccountDto[];
}

export interface AuthState {
  /** All active login sessions. */
  sessions: Map<string, SessionEntry>;
  /** Currently active/selected session ID. */
  activeSessionId: string | null;
  /** Whether any session is authenticated. */
  isAuthenticated: boolean;
  isLoggingIn: boolean;
  loginError: string | null;
  /** Temporarily holds credentials during TOTP flow. */
  pendingCredentials: {
    account: string;
    password: string;
    rememberPassword: boolean;
    sessionId: string;
  } | null;

  // Getters
  getActiveSession: () => SessionEntry | null;
  getActiveSessionId: () => string | null;
  getActiveGameAccounts: () => GameAccountDto[];

  // Actions
  addSession: (session: SessionDto, gameAccounts?: GameAccountDto[]) => void;
  removeSession: (sessionId: string) => void;
  setActiveSessionId: (sessionId: string | null) => void;
  updateGameAccounts: (sessionId: string, accounts: GameAccountDto[]) => void;
  clearAllSessions: () => void;
  setLoggingIn: (loading: boolean) => void;
  setLoginError: (error: string | null) => void;
  setPendingCredentials: (
    creds: {
      account: string;
      password: string;
      rememberPassword: boolean;
      sessionId: string;
    } | null,
  ) => void;

  // Legacy compat — delegates to active session
  /** @deprecated Use getActiveSession() instead */
  session: SessionDto | null;
  /** @deprecated Use addSession() instead */
  setSession: (session: SessionDto | null) => void;
  /** @deprecated Use clearAllSessions() instead */
  clearSession: () => void;
  /** @deprecated Use getActiveGameAccounts() instead */
  gameAccounts: GameAccountDto[];
  /** @deprecated Use updateGameAccounts() instead */
  setGameAccounts: (accounts: GameAccountDto[]) => void;
}

export const useAuthStore = create<AuthState>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,
  isAuthenticated: false,
  isLoggingIn: false,
  loginError: null,
  pendingCredentials: null,

  // Legacy compat fields (computed from active session)
  session: null,
  gameAccounts: [],

  getActiveSession: () => {
    const { sessions, activeSessionId } = get();
    if (!activeSessionId) return null;
    return sessions.get(activeSessionId) ?? null;
  },

  getActiveSessionId: () => get().activeSessionId,

  getActiveGameAccounts: () => {
    const entry = get().getActiveSession();
    return entry?.gameAccounts ?? [];
  },

  addSession: (session, gameAccounts = []) => {
    set((state) => {
      const newSessions = new Map(state.sessions);
      newSessions.set(session.sessionId, {
        sessionId: session.sessionId,
        session,
        gameAccounts,
      });
      return {
        sessions: newSessions,
        activeSessionId: session.sessionId,
        isAuthenticated: true,
        loginError: null,
        // Legacy compat
        session,
        gameAccounts,
      };
    });
  },

  removeSession: (sessionId) => {
    set((state) => {
      const newSessions = new Map(state.sessions);
      newSessions.delete(sessionId);
      const newActive =
        state.activeSessionId === sessionId
          ? (newSessions.keys().next().value ?? null)
          : state.activeSessionId;
      const activeEntry = newActive ? newSessions.get(newActive) : null;
      return {
        sessions: newSessions,
        activeSessionId: newActive,
        isAuthenticated: newSessions.size > 0,
        // Legacy compat
        session: activeEntry?.session ?? null,
        gameAccounts: activeEntry?.gameAccounts ?? [],
      };
    });
  },

  setActiveSessionId: (sessionId) => {
    set((state) => {
      const entry = sessionId ? state.sessions.get(sessionId) : null;
      return {
        activeSessionId: sessionId,
        // Legacy compat
        session: entry?.session ?? null,
        gameAccounts: entry?.gameAccounts ?? [],
      };
    });
  },

  updateGameAccounts: (sessionId, accounts) => {
    set((state) => {
      const newSessions = new Map(state.sessions);
      const entry = newSessions.get(sessionId);
      if (entry) {
        newSessions.set(sessionId, { ...entry, gameAccounts: accounts });
      }
      const isActive = state.activeSessionId === sessionId;
      return {
        sessions: newSessions,
        gameAccounts: isActive ? accounts : state.gameAccounts,
      };
    });
  },

  clearAllSessions: () =>
    set({
      sessions: new Map(),
      activeSessionId: null,
      isAuthenticated: false,
      isLoggingIn: false,
      loginError: null,
      pendingCredentials: null,
      session: null,
      gameAccounts: [],
    }),

  setLoggingIn: (isLoggingIn) => set({ isLoggingIn }),
  setLoginError: (loginError) => set({ loginError }),
  setPendingCredentials: (pendingCredentials) => set({ pendingCredentials }),

  // Legacy compat setters
  setSession: (session) => {
    if (session) {
      get().addSession(session);
    } else {
      const active = get().activeSessionId;
      if (active) get().removeSession(active);
    }
  },
  clearSession: () => {
    const active = get().activeSessionId;
    if (active) {
      get().removeSession(active);
    } else {
      get().clearAllSessions();
    }
  },
  setGameAccounts: (accounts) => {
    const active = get().activeSessionId;
    if (active) get().updateGameAccounts(active, accounts);
    else set({ gameAccounts: accounts });
  },
}));
