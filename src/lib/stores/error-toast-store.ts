import { create } from "zustand";

export interface Toast {
  id: string;
  message: string;
  category: string;
  critical: boolean;
}

export interface ErrorToastState {
  toasts: Toast[];
  addToast: (toast: Omit<Toast, "id">) => string;
  removeToast: (id: string) => void;
  /** Drop every toast of one category, e.g. auth errors once signed in again. */
  clearCategory: (category: string) => void;
}

let nextId = 0;

export const useErrorToastStore = create<ErrorToastState>((set) => ({
  toasts: [],
  addToast: (toast) => {
    const id = String(++nextId);
    set((state) => ({
      toasts: [...state.toasts, { ...toast, id }],
    }));
    return id;
  },
  removeToast: (id) =>
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    })),
  clearCategory: (category) =>
    set((state) => ({
      toasts: state.toasts.filter((t) => t.category !== category),
    })),
}));
