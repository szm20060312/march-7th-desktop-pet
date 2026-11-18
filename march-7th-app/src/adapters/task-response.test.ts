import { expect, it, vi } from "vitest";
import { connectTaskResponses } from "./tauri-reminders";
it("responds once to a live content-free task edge, never to a duplicate or invalid payload", async () => {
  let receive: (value: unknown) => void = () => {}; const respond=vi.fn(); const reportError=vi.fn();
  const dispose=connectTaskResponses({respond,reportError,transport:{invoke:vi.fn(),listen:async (_name, callback)=>{receive=callback;return () => {};}}});
  await Promise.resolve();
  receive({revision:2,status:"completed"});receive({revision:2,status:"completed"});receive({revision:1,status:"abandoned"});
  receive({revision:3,status:"active"});receive({revision:4,status:"abandoned",name:"private"});
  expect(respond.mock.calls).toEqual([["completed"]]); expect(reportError).toHaveBeenCalledTimes(2);
  receive({revision:5,status:"abandoned"});expect(respond).toHaveBeenLastCalledWith("abandoned");
  dispose();receive({revision:6,status:"completed"});expect(respond).toHaveBeenCalledTimes(2);
});
