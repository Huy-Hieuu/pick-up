// Browser tokens live only in memory. Never persist bearer tokens in localStorage.
let session: string | null = null;
export const readSession = async () => session;
export const writeSession = async (value: string | null) => { session = value; };
