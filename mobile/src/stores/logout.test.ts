import { describe, expect, it, vi } from 'vitest';
import { endSession } from './logout';
describe('logout revocation', () => {
  it('clears local access without waiting for remote revocation and surfaces remote failure', async () => {
    let fail!: (reason: unknown) => void;
    let local = true;
    const revoke = vi.fn(() => { expect(local).toBe(true); return new Promise<void>((_resolve, reject) => { fail = reject; }); });
    const clear = vi.fn(async () => { local = false; });
    const pending = endSession(revoke, clear);
    const failure = new Error('remote revocation failed');
    const assertion = expect(pending).rejects.toBe(failure);
    expect(local).toBe(false);
    expect(clear).toHaveBeenCalledOnce();
    fail(failure);
    await assertion;
    expect(local).toBe(false);
  });
  it('successfully waits for both local persistence and remote revocation', async () => {
    const revoke = vi.fn(async () => undefined);
    const clear = vi.fn(async () => undefined);
    await expect(endSession(revoke, clear)).resolves.toBeUndefined();
    expect(revoke).toHaveBeenCalledOnce();
    expect(clear).toHaveBeenCalledOnce();
  });
  it('handles remote rejection while local storage also fails', async () => {
    const remote = new Error('remote');
    const storage = new Error('storage');
    await expect(endSession(async () => { throw remote; }, async () => { throw storage; })).rejects.toBe(storage);
  });
});
