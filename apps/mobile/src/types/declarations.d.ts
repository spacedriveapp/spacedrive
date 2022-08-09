declare module '*.svg' {
	import React from 'react';
	import { SvgProps } from 'react-native-svg';
	const content: React.FC<SvgProps>;
	export default content;
}

// This declaration is used by useNavigation, Link, ref etc.
declare global {
	namespace ReactNavigation {
		interface RootParamList extends RootStackParamList {}
	}
}
