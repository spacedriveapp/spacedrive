import { proxy } from 'valtio';

// Store to cache notes for multiple files
export const noteCacheStore = proxy<{ [id: string]: string | undefined }>({});
