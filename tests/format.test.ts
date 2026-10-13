import { describe, it, expect } from 'vitest';
import { bytes, parentRemote, joinRemote, percent } from '../src/lib/format';
describe('remote paths and unavailable counters',()=>{
  it('keeps root valid and does not build double separators',()=>{expect(parentRemote('/')).toBe('/');expect(parentRemote('/var/log/')).toBe('/var');expect(joinRemote('/','中文 空格.txt')).toBe('/中文 空格.txt');});
  it('distinguishes an unavailable rate from a measured zero',()=>{expect(bytes(null)).toBe('—');expect(bytes(0)).toBe('0 B');expect(percent(null)).toBe('—');expect(percent(0)).toBe('0.0%');});
});
