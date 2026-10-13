export function bytes(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return '—';
  if (value < 1024) return `${Math.round(value)} B`;
  const units = ['KiB', 'MiB', 'GiB', 'TiB']; let n = value / 1024; let unit = 0;
  while (n >= 1024 && unit < units.length - 1) { n /= 1024; unit++; }
  return `${n.toFixed(n >= 100 ? 0 : 1)} ${units[unit]}`;
}
export function percent(value: number | null | undefined): string { return value == null || !Number.isFinite(value) ? '—' : `${value.toFixed(1)}%`; }
export function joinRemote(dir: string, name: string): string { return `${dir.replace(/\/$/, '')}/${name}`; }
export function parentRemote(dir: string): string { const parts = dir.split('/').filter(Boolean); parts.pop(); return `/${parts.join('/')}`; }
export function remoteTime(seconds: number): string { return seconds ? new Date(seconds * 1000).toLocaleString('zh-CN', { hour12: false }) : '—'; }
