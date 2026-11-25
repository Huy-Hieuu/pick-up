/** Revoke the captured bearer session while ending local access immediately. */
export async function endSession(revoke: () => Promise<unknown>, clear: () => Promise<void>): Promise<void> {
  const outcome = revoke().then(
    () => ({ok: true as const}),
    (error: unknown) => ({ok: false as const, error}),
  );
  await clear();
  const result = await outcome;
  if (!result.ok) throw result.error;
}
