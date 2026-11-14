/// <reference types="vite/client" />

interface ImportMetaEnv {
	/** "true" in builds made by the release workflow, which turns on the in-app updater. */
	readonly VITE_SD_UPDATER?: string;
}

declare module '*.svg' {
	const src: string;
	export default src;
	export const ReactComponent: React.FC<React.SVGProps<SVGSVGElement>>;
}

declare module '@mkkellogg/gaussian-splats-3d' {
	const GaussianSplats3D: any;
	export default GaussianSplats3D;
	export const Viewer: any;
	export const DropInViewer: any;
	export const SceneFormat: any;
}

declare module 'qrcode' {
	export function toCanvas(canvas: HTMLCanvasElement, text: string, options?: any): Promise<void>;
	export function toDataURL(text: string, options?: any): Promise<string>;
	export function toString(text: string, options?: any): Promise<string>;
}

interface SpacedriveGlobal {
	showContextMenu?: (
		items: import("@sd/interface").ContextMenuItem[],
		position: { x: number; y: number }
	) => Promise<void>;
	registerKeybind?: (
		id: string,
		accelerator: string,
		handler: () => void | Promise<void>
	) => Promise<void>;
	unregisterKeybind?: (id: string) => Promise<void>;
}

interface Window {
	__SPACEDRIVE__: SpacedriveGlobal;
}
