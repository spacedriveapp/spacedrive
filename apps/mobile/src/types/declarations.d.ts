import 'valtio';

declare module '*.svg' {
	import React from 'react';
	import { SvgProps } from 'react-native-svg';
	const content: React.FC<SvgProps>;
	export default content;
}

// Loosen the type definition of the `useSnapshot` hook
declare module 'valtio' {
	function useSnapshot<T extends object>(p: T): T;
}
