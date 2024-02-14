import { useBottomTabBarHeight } from '@react-navigation/bottom-tabs';
import { ReactNode } from 'react';
import { ScrollView, View } from 'react-native';
import { ClassInput } from 'twrnc/dist/esm/types';
import { twStyle } from '~/lib/tailwind';

interface Props {
	children: ReactNode;
	scrollview?: boolean;
	style?: ClassInput;
	tabHeight?: boolean;
}

const ScreenContainer = ({ children, style, scrollview = true, tabHeight = true }: Props) => {
	const height = useBottomTabBarHeight();
	return scrollview ? (
		<ScrollView
			contentContainerStyle={twStyle('justify-between gap-6 py-5', style)}
			style={twStyle('flex-1 bg-mobile-screen', { marginBottom: tabHeight && height })}
		>
			{children}
		</ScrollView>
	) : (
		<View
			style={twStyle('flex-1 justify-between gap-6 bg-mobile-screen py-5', style, {
				marginBottom: tabHeight && height
			})}
		>
			{children}
		</View>
	);
};

export default ScreenContainer;
