import { useEffect } from "react";
import { create } from "zustand";

/**
 * Which modifier keys are down right now, tracked at the window.
 *
 * dnd-kit delivers no modifier state at drag end, so a drop that means
 * something different with a key held reads the key from here instead.
 * Losing window focus releases every key, since a keyup fired elsewhere
 * never arrives.
 */
export interface ModifierKeys {
	alt: boolean;
	shift: boolean;
	meta: boolean;
	ctrl: boolean;
}

interface ModifierKeysStore extends ModifierKeys {
	set: (keys: ModifierKeys) => void;
}

const released: ModifierKeys = { alt: false, shift: false, meta: false, ctrl: false };

export const useModifierKeysStore = create<ModifierKeysStore>((set) => ({
	...released,
	set: (keys) => set(keys),
}));

function fromEvent(event: KeyboardEvent | MouseEvent): ModifierKeys {
	return {
		alt: event.altKey,
		shift: event.shiftKey,
		meta: event.metaKey,
		ctrl: event.ctrlKey,
	};
}

/**
 * Install the window listeners that feed the store. Mount once, near the
 * drag-and-drop provider; every reader then uses `useModifierKeys` or reads
 * the store directly from an event handler.
 */
export function useModifierKeyTracker() {
	const set = useModifierKeysStore((state) => state.set);

	useEffect(() => {
		const update = (event: KeyboardEvent | MouseEvent) => set(fromEvent(event));
		const release = () => set(released);
		window.addEventListener("keydown", update, true);
		window.addEventListener("keyup", update, true);
		// A pointer event carries the modifiers too, which keeps the store
		// right when a key changed while the window had no focus.
		window.addEventListener("pointermove", update, { capture: true, passive: true });
		window.addEventListener("blur", release);
		return () => {
			window.removeEventListener("keydown", update, true);
			window.removeEventListener("keyup", update, true);
			window.removeEventListener("pointermove", update, true);
			window.removeEventListener("blur", release);
		};
	}, [set]);
}

export function useModifierKeys(): ModifierKeys {
	return useModifierKeysStore((state) => ({
		alt: state.alt,
		shift: state.shift,
		meta: state.meta,
		ctrl: state.ctrl,
	}));
}

/** The keys down at this instant, for a handler outside React's render. */
export function modifierKeysNow(): ModifierKeys {
	const { alt, shift, meta, ctrl } = useModifierKeysStore.getState();
	return { alt, shift, meta, ctrl };
}
