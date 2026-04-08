import { create } from "zustand";

export interface Toast {
  id: string;
  message: string;
  category: string;
  critical: boolean;
  createdAt: number;
}

export interface ErrorToastState {
  toasts: Toast[];
  addToast: (toast: Omit<Toast, "id" | "createdAt">) => void;
  removeToast: (id: string) => void;
  clearAll: () => void;
}

let nextId = 0;

export const useErrorToastStore = create<ErrorToastState>((set) => ({
  toasts: [],
  addToast: (toast) =>
    set((state) => ({
      toasts: [
        ...state.toasts,
        {
          ...toast,
          id: String(++nextId),
          createdAt: Date.now(),
        },
      ],
    })),
  removeToast: (id) =>
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    })),
  clearAll: () => set({ toasts: [] }),
}));
