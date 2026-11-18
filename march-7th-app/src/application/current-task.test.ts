import { expect, it, vi } from "vitest";
import { createFocusControls } from "./focus-controls";
it("M5B starts with a trimmed optional current task", async () => {
  const command = vi.fn(async () => ({revision:2, data:{version:2 as const, session:{status:"running" as const,duration_ms:60000,remaining_ms:60000,anchor_utc_ms:0},task:{name:"阅读",status:"active" as const}},error:null,stopped:false}));
  const controls = createFocusControls({command,render:vi.fn(),now:()=>0});
  controls.receive({snapshot:{revision:1,data:{version:2,session:{status:"idle"},task:null},error:null,stopped:false},completedNow:false,error:null});
  controls.setDuration(1); controls.setTaskName("  阅读  "); await controls.act("start");
  expect(command).toHaveBeenCalledWith({type:"start",durationMs:60000,taskName:"阅读"});
});
import { normalizeTaskName } from "../domain/focus";
it("validates Unicode scalar, UTF-8 and control boundaries without logging content", () => {
  for (const name of ["中".repeat(80),"😀".repeat(64),"a".repeat(80),"\ufeff"]) expect(normalizeTaskName(name)).toBe(name);
  expect(normalizeTaskName("　 ")).toBe("");
  for (const name of ["a".repeat(81),"😀".repeat(65),"secret\n", "\t", "\u0085", "\ud800"]) expect(()=>normalizeTaskName(name)).toThrow("invalidTaskName");
});
it("rejects invalid names before command and keeps task resolution independent from running time", async () => {
  let state: any; const command=vi.fn(async()=>({revision:3,data:{version:2 as const,session:{status:"running" as const,duration_ms:60000,remaining_ms:60000,anchor_utc_ms:0},task:{name:"阅读",status:"completed" as const}},error:null,stopped:false}));
  const controls=createFocusControls({command,render:s=>state=s,now:()=>0});
  controls.receive({snapshot:{revision:1,data:{version:2,session:{status:"idle"},task:null},error:null,stopped:false},completedNow:false,error:null});
  controls.setTaskName("secret\n");await controls.act("start");expect(command).not.toHaveBeenCalled();expect(state.notice).not.toContain("secret");
  controls.receive({snapshot:{revision:2,data:{version:2,session:{status:"running",duration_ms:60000,remaining_ms:60000,anchor_utc_ms:0},task:{name:"阅读",status:"active"}},error:null,stopped:false},completedNow:false,error:null});
  await controls.act("completeTask");await controls.act("abandonTask");expect(command).toHaveBeenCalledExactlyOnceWith({type:"completeTask"});expect(state.snapshot.data.session.status).toBe("running");
  controls.close();controls.reopen();expect(command).toHaveBeenCalledTimes(1);
});
