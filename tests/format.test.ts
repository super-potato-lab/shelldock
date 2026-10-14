import { describe, it, expect } from 'vitest';
import { bytes, parentRemote, joinRemote, percent, permissionsText } from '../src/lib/format';
describe('remote paths and unavailable counters',()=>{
  it('keeps root valid and does not build double separators',()=>{expect(parentRemote('/')).toBe('/');expect(parentRemote('/var/log/')).toBe('/var');expect(joinRemote('/','中文 空格.txt')).toBe('/中文 空格.txt');});
  it('distinguishes an unavailable rate from a measured zero',()=>{expect(bytes(null)).toBe('—');expect(bytes(0)).toBe('0 B');expect(percent(null)).toBe('—');expect(percent(0)).toBe('0.0%');});
});

describe('Unix file permissions',()=>{
  it('preserves directory, symlink and special execute bits',()=>{
    expect(permissionsText(0o40755,true,false)).toBe('drwxr-xr-x');
    expect(permissionsText(0o100644,false,false)).toBe('-rw-r--r--');
    expect(permissionsText(0o120777,false,true)).toBe('lrwxrwxrwx');
    expect(permissionsText(0o4755,false,false)).toBe('-rwsr-xr-x');
    expect(permissionsText(0o2644,false,false)).toBe('-rw-r-Sr--');
    expect(permissionsText(0o1777,true,false)).toBe('drwxrwxrwt');
    expect(permissionsText(null,false,false)).toBe('—');
  });
});
