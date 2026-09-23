import { invoke } from "@tauri-apps/api/core";

export interface BuildInfo {
  schemaVersion: 1;
  appVersion: string;
  target: string;
  sourceCommit: string | null;
  sourceState: "clean" | "modified" | "unknown";
}

export function parseBuildInfo(value: unknown): BuildInfo {
  if (!value || typeof value !== "object") throw Error("Invalid build identity");
  const info = value as Record<string, unknown>;
  if (info.schemaVersion !== 1
    || typeof info.appVersion !== "string" || info.appVersion.length > 64 || !/^\d+\.\d+\.\d+(?:[-+][\w.-]+)?$/.test(info.appVersion)
    || typeof info.target !== "string" || info.target.length > 128 || !/^[a-z0-9_]+(?:-[a-z0-9_]+)+$/.test(info.target)
    || !["clean", "modified", "unknown"].includes(info.sourceState as string)
    || (info.sourceState === "unknown" ? info.sourceCommit !== null : typeof info.sourceCommit !== "string" || !/^[a-f0-9]{40}$/.test(info.sourceCommit))) {
    throw Error("Invalid build identity");
  }
  return { schemaVersion: 1, appVersion: info.appVersion, target: info.target, sourceCommit: info.sourceCommit as string | null, sourceState: info.sourceState as BuildInfo["sourceState"] };
}

export async function readBuildInfo(): Promise<BuildInfo> {
  return parseBuildInfo(await invoke<unknown>("get_build_info"));
}
