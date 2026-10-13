import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Connection } from './types';
export const desktop = isTauri();
let previewConnections: Connection[] = [];
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (desktop) return invoke<T>(command, args);
  if (command === 'bootstrap') return { connections: previewConnections, history: [], quickCommands: ['ls -lah', 'df -h', 'free -m', 'top', 'pwd'] } as T;
  if (command === 'save_connection') {
    const c = { ...(args?.connection as Connection), id: (args?.connection as Connection).id || crypto.randomUUID(), hasPassword: false };
    previewConnections = [...previewConnections.filter(p => p.id !== c.id), c]; return c as T;
  }
  if (command === 'delete_connection') { previewConnections = previewConnections.filter(c => c.id !== args?.id); return undefined as T; }
  if (command === 'save_quick_commands' || command === 'record_command') return undefined as T;
  throw new Error('请在 ShellDock 桌面应用中使用此功能。浏览器当前仅显示界面预览。');
}
