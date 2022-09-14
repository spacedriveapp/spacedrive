declare type KeybindEvent<T = string> = CustomEvent<{ action: T }>;

interface GlobalEventHandlersEventMap {
	exec_keybind: CustomEvent<{ action: string }>;
}
