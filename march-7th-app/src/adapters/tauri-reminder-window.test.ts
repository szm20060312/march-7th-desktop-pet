import { expect, it, vi } from "vitest";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ hide: async () => {} }) }));
import { hideSettingsWindow } from "./tauri-reminder-window";
it("closes settings through the native lifecycle fence, without a target label", async () => {
  invoke.mockResolvedValue(undefined);
  await hideSettingsWindow();
  expect(invoke).toHaveBeenCalledExactlyOnceWith("hide_reminder_settings");
});
it("retains native close failure for the entry to report", async () => {
  invoke.mockRejectedValue(new Error("hide failed"));
  await expect(hideSettingsWindow()).rejects.toThrow("hide failed");
});
