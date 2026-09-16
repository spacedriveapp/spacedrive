import {useSyncExternalStore} from 'react';

export type ShellLayoutMode = 'inset' | 'floating';

const STORAGE_KEY = 'spacedrive-shell-layout';
const DEFAULT_LAYOUT: ShellLayoutMode = 'inset';
const listeners = new Set<() => void>();

function parseLayout(value: string | null): ShellLayoutMode {
	return value === 'floating' ? 'floating' : DEFAULT_LAYOUT;
}

function readLayout(fallback = DEFAULT_LAYOUT): ShellLayoutMode {
	try {
		return parseLayout(window.localStorage.getItem(STORAGE_KEY));
	} catch {
		return fallback;
	}
}

let layout = readLayout();

function notify() {
	for (const listener of listeners) listener();
}

function handleStorage(event: StorageEvent) {
	if (event.key !== STORAGE_KEY && event.key !== null) return;
	layout = readLayout(layout);
	notify();
}

function subscribe(listener: () => void) {
	if (listeners.size === 0) {
		layout = readLayout(layout);
		window.addEventListener('storage', handleStorage);
	}
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
		if (listeners.size === 0) {
			window.removeEventListener('storage', handleStorage);
		}
	};
}

/** Keep layout local to this client, including its separate Settings window. */
export function setShellLayout(next: ShellLayoutMode) {
	layout = next;
	try {
		window.localStorage.setItem(STORAGE_KEY, next);
	} catch {
		// Retain the selection for this session when storage is unavailable.
	}
	notify();
}

export function useShellLayout() {
	return useSyncExternalStore(
		subscribe,
		() => layout,
		() => DEFAULT_LAYOUT
	);
}
