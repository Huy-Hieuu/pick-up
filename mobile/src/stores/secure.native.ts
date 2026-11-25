import * as SecureStore from 'expo-secure-store';
const key = 'pickup.session.v1';
export const readSession = () => SecureStore.getItemAsync(key);
export const writeSession = (value: string | null) => value === null ? SecureStore.deleteItemAsync(key) : SecureStore.setItemAsync(key, value);
