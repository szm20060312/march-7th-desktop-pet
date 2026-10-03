import { describe, expect, it } from "vitest";
import { parseBuildInfo } from "./build-info";

const info = { schemaVersion: 1, appVersion: "0.2.0", target: "x86_64-pc-windows-msvc", sourceCommit: "a".repeat(40), sourceState: "clean" };
describe("build identity boundary", () => {
  it("accepts clean, modified and honest unknown provenance", () => {
    expect(parseBuildInfo(info)).toEqual(info);
    expect(parseBuildInfo({ ...info, sourceState: "modified" }).sourceState).toBe("modified");
    expect(parseBuildInfo({ ...info, sourceCommit: null, sourceState: "unknown" }).sourceCommit).toBeNull();
  });
  it.each([
    null, { ...info, schemaVersion: 2 }, { ...info, sourceState: "trusted" },
    { ...info, sourceCommit: "A".repeat(40) }, { ...info, sourceCommit: null },
    { ...info, sourceState: "unknown" }, { ...info, appVersion: "" },
    { ...info, target: "x".repeat(129) }, { ...info, appVersion: "<img>" },
  ])("rejects malformed or contradictory identity: %j", value => {
    expect(() => parseBuildInfo(value)).toThrow("Invalid build identity");
  });
});
