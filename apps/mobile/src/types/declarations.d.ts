declare module '*.svg' {
	import React from 'react';
	import { SvgProps } from 'react-native-svg';
	// TODO: This is probably not working as intended
	export const ReactComponent: React.FC<SVGProps<SVGSVGElement>>;
}

declare module '*.png' {
	const content: any;
	export default content;
}

declare module '*.mp4' {
	const content: any;
	export default content;
}

declare module '*.webm' {
	const content: any;
	export default content;
}
