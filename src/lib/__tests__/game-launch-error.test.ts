import { beforeEach, describe, expect, it, vi } from "vitest";
import { reportGameLaunchError } from "../game-launch-error";
import { useErrorToastStore } from "../stores/error-toast-store";
import { commands } from "../tauri";

vi.mock("../tauri", () => ({
  commands: { openFileDialog: vi.fn(), setConfig: vi.fn() },
}));

describe("missing game executable", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useErrorToastStore.setState({ toasts: [] });
  });

  it("lets the player choose and save a path from the launch prompt", async () => {
    const path = String.raw`D:\Games\MapleStory.exe`;
    vi.mocked(commands.openFileDialog).mockResolvedValue(path);
    reportGameLaunchError({ code: "PROC_GAME_PATH_NOT_FOUND" });

    const [toast] = useErrorToastStore.getState().toasts;
    expect(toast?.critical).toBe(true);
    await toast?.action?.onClick();

    expect(commands.setConfig).toHaveBeenCalledWith("game_path", path);
    expect(useErrorToastStore.getState().toasts).toEqual([]);
  });

  it("keeps the prompt and leaves the path unchanged when the picker is cancelled", async () => {
    vi.mocked(commands.openFileDialog).mockResolvedValue(null);
    reportGameLaunchError({ code: "PROC_GAME_PATH_NOT_FOUND" });

    const [toast] = useErrorToastStore.getState().toasts;
    await toast?.action?.onClick();

    expect(commands.openFileDialog).toHaveBeenCalledOnce();
    expect(commands.setConfig).not.toHaveBeenCalled();
    expect(useErrorToastStore.getState().toasts).toEqual([toast]);
  });
});
