import { useCallback, useState } from 'react';
import type { Icon } from '@phosphor-icons/react';
import { usePlatform } from '../platform';

export interface ContextMenuItem {
	type?: 'separator' | 'submenu';
	icon?: Icon;
	label?: string;
	onClick?: () => void;
	keybind?: string;
	variant?: 'default' | 'dull' | 'danger';
	disabled?: boolean;
	condition?: () => boolean;
	submenu?: ContextMenuItem[];
}

export interface ContextMenuConfig {
	items: ContextMenuItem[];
}

export interface ContextMenuResult {
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
export function useContextMenu(config: ContextMenuConfig): ContextMenuResult {
	const [menuData, setMenuData] = useState<ContextMenuItem[] | null>(null);
	const platform = usePlatform();

	const show = useCallback(
		async (e: React.MouseEvent) => {
			console.log('[useContextMenu] show called', { x: e.clientX, y: e.clientY });
			e.preventDefault();
			e.stopPropagation();

			// Filter items by condition
			const visibleItems = config.items.filter(
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
		[config.items, platform]
	);

	const closeMenu = useCallback(() => {
		setMenuData(null);
	}, []);

	return { show, menuData, closeMenu };
}
