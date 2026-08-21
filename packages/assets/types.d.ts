// Type declarations for @sd/assets

declare module "@sd/assets/icons/*.webp" {
	const value: number; // React Native uses numeric IDs for local images
	export default value;
}

declare module "@sd/assets/icons/*.jpg" {
	const value: number;
	export default value;
}

declare module "@sd/assets/images/*.webp" {
	const value: number;
	export default value;
}

declare module "@sd/assets/images/*.jpg" {
	const value: number;
	export default value;
}

declare module "@sd/assets/svgs/*.svg" {
	import type { FC } from "react";
	const content: FC<Record<string, unknown>>;
	export default content;
}

declare module "*.svg" {
	const src: string;
	export default src;
	export const ReactComponent: React.FC<React.SVGProps<SVGSVGElement>>;
}

declare module "@sd/assets/videos/*.mp4" {
	const value: number;
	export default value;
}

declare module "@sd/assets/sounds/*.mp3" {
	const value: number | string; // number on React Native (asset ID), string on web (URL)
	export default value;
}

declare module "@sd/assets/lottie/*.json" {
	const value: object;
	export default value;
}
