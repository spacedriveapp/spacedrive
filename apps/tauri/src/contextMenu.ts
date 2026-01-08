import type { ContextMenuItem } from "@sd/interface";
import {
  Menu,
  MenuItem,
  PredefinedMenuItem,
  Submenu,
} from "@tauri-apps/api/menu";

/**
 * Convert platform-agnostic menu items to Tauri's native Menu API
 */
export async function showNativeContextMenu(
  items: ContextMenuItem[],
  position: { x: number; y: number }
) {
  console.log("[Tauri ContextMenu] Building native menu from items:", items);

  const menuItems = await buildMenuItems(items);
  const menu = await Menu.new({ items: menuItems });

  console.log("[Tauri ContextMenu] Showing menu at position:", position);
  await menu.popup();
}

/**
 * Recursively build Tauri menu items from platform-agnostic definitions
 */
async function buildMenuItems(items: ContextMenuItem[]): Promise<any[]> {
  const menuItems = [];

  for (const item of items) {
    if (item.type === "separator") {
      // Add separator
      menuItems.push(await PredefinedMenuItem.new({ item: "Separator" }));
    } else if (item.submenu) {
      // Add submenu
      const subItems = await buildMenuItems(item.submenu);
      const submenu = await Submenu.new({
        text: item.label || "Submenu",
        items: subItems,
      });
      menuItems.push(submenu);
    } else {
      // Add regular menu item
      const menuItem = await MenuItem.new({
        text: item.label || "",
        enabled: !item.disabled,
        accelerator: item.keybind,
        action: item.onClick,
      });
      menuItems.push(menuItem);
    }
  }

  return menuItems;
}

/**
 * Initialize the context menu handler on the window global
 */
export function initializeContextMenuHandler() {
  if (!window.__SPACEDRIVE__) {
    (window as any).__SPACEDRIVE__ = {};
  }

  window.__SPACEDRIVE__.showContextMenu = showNativeContextMenu;
  console.log("[Tauri ContextMenu] Handler initialized");
}
