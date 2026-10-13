export interface Connection {
  id: string; name: string; host: string; port: number; username: string; group: string;
  auth: 'password' | 'key'; privateKeyPath: string | null; notes: string; hasPassword: boolean;
  protocol: string; extra: Record<string, unknown> | null;
}
export interface FileEntry { name: string; path: string; isDir: boolean; isSymlink: boolean; size: number; modified: number; permissions: number | null; }
export interface Revision { size: number; modified: number; }
export interface RemoteText { path: string; content: string; revision: Revision; }
export interface Metrics {
  system: string; timestamp: number; uptime: number; cpuPercent: number | null;
  memoryTotal: number; memoryAvailable: number; swapTotal: number; swapFree: number;
  load: number[];
  interfaces: { name: string; received: number; sent: number; receiveRate: number | null; sendRate: number | null }[];
  disks: { name: string; mount: string; total: number; used: number; available: number }[];
  processes: { pid: number; user: string; cpu: number; memory: number; command: string }[];
}
export interface Tab {
  id: string; connection: Connection; sessionId?: string;
  status: 'idle' | 'connecting' | 'connected' | 'closed' | 'error';
  path: string; metrics: Metrics | null; metricError: string; pendingAck: number;
}
export interface Transfer { id: string; name: string; direction: string; transferred: number; total: number; status: string; error: string | null; }
export type SessionEvent = {event: 'output'; data: number[]} | {event: 'closed'} | {event: 'error'; data: string};
export type ConnectResult = {status: 'connected'; sessionId: string; path: string} | {status: 'needsTrust'; fingerprint: string; changed: boolean};
