import { readBuildInfo } from "./build-info";

// One read, independent of the reminder connection and its editable draft.
export function mountBuildInfo(document: Document): () => void {
  let disposed = false;
  const text = (id: string, value: string) => { document.getElementById(id)!.textContent = value; };
  void readBuildInfo().then(info => {
    if (disposed) return;
    text("build-version", info.appVersion);
    text("build-target", info.target);
    text("build-commit", info.sourceCommit ?? "未知");
    text("build-state", { clean: "构建输入无本地修改", modified: "包含本地修改", unknown: "来源未知" }[info.sourceState]);
    text("build-status", "");
  }).catch(() => { if (!disposed) text("build-status", "版本信息读取失败；可重启应用后重试。"); });
  return () => { disposed = true; };
}
