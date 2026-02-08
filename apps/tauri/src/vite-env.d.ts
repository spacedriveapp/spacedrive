/// <reference types="vite/client" />

// Extend vite/client's *.svg declaration with SVGR's ReactComponent export
declare module '*.svg' {
	import type { FC, SVGProps } from 'react';
	const src: string;
	export default src;
	export const ReactComponent: FC<SVGProps<SVGSVGElement>>;
}
