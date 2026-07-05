import { invoke } from "@tauri-apps/api/core";

export type TauriInvoker = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export const tauriInvoke: TauriInvoker = <T>(command: string, args?: Record<string, unknown>) =>
  invoke<T>(command, args);
