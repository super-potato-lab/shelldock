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
export function remoteTime(seconds: number): string { return seconds ? new Date(seconds * 1000).toLocaleString('zh-CN', { hour12: false, year:'numeric', month:'2-digit', day:'2-digit', hour:'2-digit', minute:'2-digit' }) : '—'; }
export function compactBytes(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return '—';
  if (value < 1024) return Math.round(value) + 'B';
  const units = ['K', 'M', 'G', 'T']; let n = value / 1024; let unit = 0;
  while (n >= 1024 && unit < units.length - 1) { n /= 1024; unit++; }
  return Number(n.toFixed(n >= 100 ? 0 : 1)) + units[unit];
}
export function permissionsText(mode: number | null, isDir: boolean, isSymlink: boolean): string {
  if (mode == null) return '—';
  let result = isSymlink ? 'l' : isDir ? 'd' : '-';
  for (let group = 2; group >= 0; group--) {
    result += mode & (4 << (group * 3)) ? 'r' : '-';
    result += mode & (2 << (group * 3)) ? 'w' : '-';
    const execute = !!(mode & (1 << (group * 3)));
    const special = !!(mode & (group === 2 ? 0o4000 : group === 1 ? 0o2000 : 0o1000));
    result += special ? (group === 0 ? (execute ? 't' : 'T') : (execute ? 's' : 'S')) : (execute ? 'x' : '-');
  }
  return result;
}
