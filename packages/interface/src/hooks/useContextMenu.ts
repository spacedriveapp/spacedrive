import { useCallback, useMemo, useState } from 'react';
import type { Icon } from '@phosphor-icons/react';
import { usePlatform } from '../platform';
import type { KeybindId } from '../util/keybinds/registry';
import { getKeybind } from '../util/keybinds/registry';
import { getComboForPlatform, getCurrentPlatform, toDisplayString } from '../util/keybinds/platform';

export interface ContextMenuItem {
	type?: 'separator' | 'submenu';
	icon?: Icon;
	label?: string;
	onClick?: () => void;
	/** Manual keybind display string (legacy) */
	keybind?: string;
	/** Type-safe keybind ID from registry - automatically resolves to platform-specific display string */
	keybindId?: KeybindId;
	variant?: 'default' | 'dull' | 'danger';
	disabled?: boolean;
	condition?: () => boolean;
	submenu?: ContextMenuItem[];
}

export interface ContextMenuConfig {
	items: ContextMenuItem[];
}

interface ContextMenuResult {
	show: (e: React.MouseEvent) => Promise<void>;
	menuData: ContextMenuItem[] | null;
	closeMenu: () => void;
}

/**
 * Hook for creating context menus that work both natively (Tauri) and in web
 *
 * This hook is platform-agnostic. Menu items are defined once in React,
 * and the platform adapter (Tauri or Web) handles the rendering.
 *
 * Usage:
 * ```tsx
 * const contextMenu = useContextMenu({
 *   items: [
 *     {
 *       icon: Copy,
 *       label: "Copy",
 *       onClick: () => copyItems(),
 *       condition: () => selectedItems.length > 0
 *     },
 *     { type: "separator" },
 *     {
 *       label: "Delete",
 *       icon: Trash,
 *       onClick: () => deleteItems(),
 *       variant: "danger"
 *     }
 *   ]
 * });
 *
 * return <div onContextMenu={contextMenu.show}>Content</div>;
 * ```
 */
/**
 * Resolve keybindId to display string for a menu item
 */
function resolveKeybindDisplay(item: ContextMenuItem): ContextMenuItem {
	// If keybindId is provided, automatically resolve to display string
	if (item.keybindId && !item.keybind) {
		const keybind = getKeybind(item.keybindId);
		if (keybind) {
			const platform = getCurrentPlatform();
			const combo = getComboForPlatform(keybind.combo, platform);
			const displayString = toDisplayString(combo, platform);
			return { ...item, keybind: displayString };
		}
	}

	// Recursively process submenus
	if (item.submenu) {
		return {
			...item,
			submenu: item.submenu.map(resolveKeybindDisplay),
		};
	}

	return item;
}

export function useContextMenu(config: ContextMenuConfig): ContextMenuResult {
	const [menuData, setMenuData] = useState<ContextMenuItem[] | null>(null);
	const platform = usePlatform();

	// Pre-process items to resolve keybindIds
	const processedItems = useMemo(
		() => config.items.map(resolveKeybindDisplay),
		[config.items]
	);

	const show = useCallback(
		async (e: React.MouseEvent) => {
			console.log('[useContextMenu] show called', { x: e.clientX, y: e.clientY });
			e.preventDefault();
			e.stopPropagation();

			// Filter items by condition
			const visibleItems = processedItems.filter(
				(item) => !item.condition || item.condition()
			);

			console.log('[useContextMenu] visible items:', visibleItems.length);

			// Check if running in Tauri
			const isTauri = platform.platform === 'tauri';
			console.log('[useContextMenu] isTauri:', isTauri);

			if (isTauri) {
				// Native mode: Use Tauri's native menu API
				console.log('[useContextMenu] Using Tauri native menu');

				try {
					// Call the platform-specific context menu handler
					// This will be provided by the Tauri app wrapper
					if (window.__SPACEDRIVE__?.showContextMenu) {
						await window.__SPACEDRIVE__.showContextMenu(visibleItems, {
							x: e.clientX,
							y: e.clientY,
						});
					} else {
						console.warn('[useContextMenu] Tauri context menu handler not found, falling back to web mode');
						setMenuData(visibleItems);
					}
				} catch (err) {
					console.error('[useContextMenu] Failed to show native context menu:', err);
					// Fallback to web mode
					setMenuData(visibleItems);
				}
			} else {
				// Web mode: Use Radix ContextMenu (trigger via state)
				console.log('[useContextMenu] Using web mode (Radix)');
				setMenuData(visibleItems);
			}
		},
		[processedItems, platform]
	);

	const closeMenu = useCallback(() => {
		setMenuData(null);
	}, []);

	return { show, menuData, closeMenu };
}
