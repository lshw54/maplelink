import { create } from "zustand";
import type { UpdateInfoDto } from "../types";

export type DownloadStatus = "idle" | "downloading" | "done" | "error";

export interface UpdateDownloadState {
  status: DownloadStatus;
  downloaded: number;
  total: number;
  speed: number; // bytes/sec
  error: string | null;
  method: string; // "direct" | proxy URL
  version: string;
  downloadUrl: string;
  isPrerelease: boolean;
  /** Cached update info from startup check — survives dialog dismiss */
  availableUpdate: UpdateInfoDto | null;

  setAvailableUpdate: (info: UpdateInfoDto | null) => void;
  startDownload: (
    version: string,
    downloadUrl: string,
    isPrerelease: boolean,
    method: string,
  ) => void;
  updateProgress: (downloaded: number, total: number, speed: number) => void;
  setDone: () => void;
  setError: (msg: string) => void;
  reset: () => void;
}

export const useUpdateStore = create<UpdateDownloadState>((set) => ({
  status: "idle",
  downloaded: 0,
  total: 0,
  speed: 0,
  error: null,
  method: "direct",
  version: "",
  downloadUrl: "",
  isPrerelease: false,
  availableUpdate: null,

  setAvailableUpdate: (info) => set({ availableUpdate: info }),
  startDownload: (version, downloadUrl, isPrerelease, method) =>
    set({
      status: "downloading",
      downloaded: 0,
      total: 0,
      speed: 0,
      error: null,
      version,
      downloadUrl,
      isPrerelease,
      method,
    }),
  updateProgress: (downloaded, total, speed) => set({ downloaded, total, speed }),
  setDone: () => set({ status: "done" }),
  setError: (msg) => set({ status: "error", error: msg }),
  reset: () =>
    set({
      status: "idle",
      downloaded: 0,
      total: 0,
      speed: 0,
      error: null,
      method: "direct",
      version: "",
      downloadUrl: "",
      isPrerelease: false,
    }),
}));
