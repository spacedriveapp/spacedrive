declare global {
	interface GlobalEventHandlersEventMap {
		keybindexec: KeybindEvent;
		deeplink: DeeplinkEvent;
		filedrop: FileDropEvent;
	}
}

export class KeybindEvent extends CustomEvent<{ action: string }> {
	constructor(action: string) {
		super('keybindexec', {
			detail: {
				action
			}
		});
	}
}

export class DeeplinkEvent extends CustomEvent<{ url: string }> {
	constructor(url: string) {
		super('deeplink', {
			detail: {
				url
			}
		});
	}
}

export class FileDropEvent extends CustomEvent<{ paths: string[] }> {
	constructor(paths: string[]) {
		super('filedrop', {
			detail: {
				paths
			}
		});
	}
}
