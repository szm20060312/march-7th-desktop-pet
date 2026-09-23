import { invoke } from "@tauri-apps/api/core";
import type { BackupResult } from "../application/local-backup";

export async function exportLocalBackup(): Promise<BackupResult> {
  const result: unknown = await invoke("export_local_backup");
  if (result === "saved" || result === "cancelled") return result;
  throw { code: "backupInvalidResponse" };
}
