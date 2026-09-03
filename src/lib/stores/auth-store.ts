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
  /** Temporarily holds credentials during TOTP flow. */
  pendingCredentials: {
    account: string;
    password: string;
    rememberPassword: boolean;
    sessionId: string;
  } | null;

  // Getters
  /** The session that owns a given game account (falls back to active). */
  sessionIdForAccount: (accountId: string) => string | null;

  // Actions
  addSession: (session: SessionDto, gameAccounts?: GameAccountDto[]) => void;
  removeSession: (sessionId: string) => void;
  setActiveSessionId: (sessionId: string | null) => void;
  updateGameAccounts: (sessionId: string, accounts: GameAccountDto[]) => void;
  renameSession: (sessionId: string, accountName: string) => void;
  setPendingCredentials: (
    creds: {
      account: string;
      password: string;
      rememberPassword: boolean;
      sessionId: string;
    } | null,
  ) => void;
}

/** Selector: the active session's DTO, or null. */
export const selectActiveSession = (s: AuthState): SessionDto | null =>
  s.activeSessionId ? (s.sessions.get(s.activeSessionId)?.session ?? null) : null;

export const useAuthStore = create<AuthState>((set, get) => ({
  sessions: new Map(),
  activeSessionId: null,
  isAuthenticated: false,
  pendingCredentials: null,

  sessionIdForAccount: (accountId) => {
    for (const [sid, entry] of get().sessions) {
      if (entry.gameAccounts.some((a) => a.id === accountId)) return sid;
    }
    // Unknown account (not in any loaded list) — fall back to the active one.
    return get().activeSessionId;
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
      return {
        sessions: newSessions,
        activeSessionId: newActive,
        isAuthenticated: newSessions.size > 0,
      };
    });
  },

  setActiveSessionId: (sessionId) => set({ activeSessionId: sessionId }),

  updateGameAccounts: (sessionId, accounts) => {
    set((state) => {
      const newSessions = new Map(state.sessions);
      const entry = newSessions.get(sessionId);
      if (entry) {
        newSessions.set(sessionId, { ...entry, gameAccounts: accounts });
      }
      return { sessions: newSessions };
    });
  },

  renameSession: (sessionId, accountName) => {
    set((state) => {
      const entry = state.sessions.get(sessionId);
      if (!entry) return state;
      const newSessions = new Map(state.sessions);
      newSessions.set(sessionId, { ...entry, session: { ...entry.session, accountName } });
      return { sessions: newSessions };
    });
  },

  setPendingCredentials: (pendingCredentials) => set({ pendingCredentials }),
}));
