import {createContext, useContext} from 'react';
import type {ShellLayoutMode} from '../stores/shellLayoutStore';

export const SHELL_SIDEBAR_WIDTH = 220;
export const SHELL_INSPECTOR_WIDTH = 280;
export const SHELL_TOOLBAR_HEIGHT = 48;

export interface ShellGeometry {
	mode: ShellLayoutMode;
	/** Insets inside the stable preview and size-view portal hosts. */
	overlayLeft: number;
	overlayRight: number;
}

export const ShellGeometryContext = createContext<ShellGeometry | null>(null);

export function useShellGeometry() {
	const geometry = useContext(ShellGeometryContext);
	if (!geometry) throw new Error('Shell geometry requires ShellLayout');
	return geometry;
}
