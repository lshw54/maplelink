import { create } from "zustand";
import type { GameAccountDto, SessionDto } from "../types";

export interface AuthState {
  session: SessionDto | null;
  isAuthenticated: boolean;
  isLoggingIn: boolean;
  loginError: string | null;
  gameAccounts: GameAccountDto[];
  /** Temporarily holds credentials during TOTP flow so they can be saved after verification. */
  pendingCredentials: { account: string; password: string; rememberPassword: boolean } | null;
  setSession: (session: SessionDto | null) => void;
  clearSession: () => void;
  setLoggingIn: (loading: boolean) => void;
  setLoginError: (error: string | null) => void;
  setGameAccounts: (accounts: GameAccountDto[]) => void;
  setPendingCredentials: (
    creds: { account: string; password: string; rememberPassword: boolean } | null,
  ) => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  session: null,
  isAuthenticated: false,
  isLoggingIn: false,
  loginError: null,
  gameAccounts: [],
  pendingCredentials: null,
  setSession: (session) => set({ session, isAuthenticated: session !== null, loginError: null }),
  clearSession: () =>
    set({
      session: null,
      isAuthenticated: false,
      isLoggingIn: false,
      loginError: null,
      gameAccounts: [],
      pendingCredentials: null,
    }),
  setLoggingIn: (isLoggingIn) => set({ isLoggingIn }),
  setLoginError: (loginError) => set({ loginError }),
  setGameAccounts: (gameAccounts) => set({ gameAccounts }),
  setPendingCredentials: (pendingCredentials) => set({ pendingCredentials }),
}));
