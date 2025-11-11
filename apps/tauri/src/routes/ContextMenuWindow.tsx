import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ContextMenu } from "@sd/ui";
import { useEffect, useRef, useState } from "react";

export interface MenuItem {
	type?: "separator";
	icon?: React.ElementType;
	label?: string;
	onClick?: () => void;
	keybind?: string;
	variant?: "default" | "dull" | "danger";
	disabled?: boolean;
	submenu?: MenuItem[];
}

export interface ContextMenuData {
	items: MenuItem[];
	x: number;
	y: number;
}

export function ContextMenuWindow() {
	const [items, setItems] = useState<MenuItem[]>([]);
	const [contextId, setContextId] = useState<string | null>(null);
	const menuRef = useRef<HTMLDivElement>(null);
	const window = getCurrentWebviewWindow();

	useEffect(() => {
		console.log('[ContextMenuWindow] Component mounted');
		console.log('[ContextMenuWindow] Window location:', window.location.href);

		// Extract context ID from URL params
		const params = new URLSearchParams(window.location.search);
		const id = params.get("context");
		console.log('[ContextMenuWindow] Context ID from params:', id);
		console.log('[ContextMenuWindow] All params:', Array.from(params.entries()));
		setContextId(id);

		if (!id) {
			console.error("[ContextMenuWindow] No context ID provided");
			return;
		}

		// Listen for menu data event
		const setupMenu = async () => {
			console.log('[ContextMenuWindow] Setting up menu listener...');
			const { listen } = await import("@tauri-apps/api/event");

			const eventName = `context-menu-data-${id}`;
			console.log('[ContextMenuWindow] Listening for event:', eventName);

			const unlisten = await listen<ContextMenuData>(
				eventName,
				(event) => {
					console.log('[ContextMenuWindow] Received menu data:', event.payload);
					const data = event.payload;
					setItems(data.items);

					// Measure actual size and adjust window after render
					requestAnimationFrame(() => {
						if (menuRef.current) {
							const { width, height } = menuRef.current.getBoundingClientRect();
							console.log('[ContextMenuWindow] Positioning menu:', { width, height, x: data.x, y: data.y });

							// Position the menu at the cursor
							invoke("position_context_menu", {
								label: window.label,
								x: data.x,
								y: data.y,
								menuWidth: width,
								menuHeight: height,
							}).catch(console.error);
						}
					});
				}
			);

			console.log('[ContextMenuWindow] Listener set up successfully');
			return unlisten;
		};

		setupMenu();

		// Close on blur (when clicking outside)
		const handleBlur = async () => {
			invoke("close_window", { label: window.label }).catch(console.error);
		};

		window.listen("tauri://blur", handleBlur);

		return () => {
			// Cleanup handled by Tauri
		};
	}, []);

	const handleItemClick = (item: MenuItem) => {
		if (item.onClick && !item.disabled) {
			item.onClick();
		}
		// Close menu after click
		invoke("close_window", { label: window.label }).catch(console.error);
	};

	const renderItem = (item: MenuItem, index: number) => {
		if (item.type === "separator") {
			return <ContextMenu.Separator key={index} />;
		}

		if (item.submenu) {
			return (
				<ContextMenu.SubMenu
					key={index}
					label={item.label || ""}
					icon={item.icon}
					variant={item.variant}
				>
					{item.submenu.map((sub, subIndex) => renderItem(sub, subIndex))}
				</ContextMenu.SubMenu>
			);
		}

		return (
			<ContextMenu.Item
				key={index}
				icon={item.icon}
				label={item.label}
				keybind={item.keybind}
				variant={item.variant}
				disabled={item.disabled}
				onClick={() => handleItemClick(item)}
			/>
		);
	};

	// Don't render until we have items
	if (items.length === 0) {
		return null;
	}

	return (
		<div
			ref={menuRef}
			className="p-1"
			style={{
				// Ensure transparent background shows through
				background: "transparent",
			}}
		>
			<div className="bg-menu/95 backdrop-blur-lg border border-menu-line rounded-lg shadow-2xl overflow-hidden">
				{items.map((item, index) => renderItem(item, index))}
			</div>
		</div>
	);
}
