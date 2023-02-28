declare global {
	interface GlobalEventHandlersEventMap {
		keybindexec: KeybindEvent;
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
