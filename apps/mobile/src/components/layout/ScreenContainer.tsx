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

const BottomTabBarHeight = 80;

const ScreenContainer = ({ children, style, scrollview = true, tabHeight = true }: Props) => {
	return scrollview ? (
		<ScrollView
			contentContainerStyle={twStyle('justify-between gap-10 py-6', style)}
			style={twStyle(
				'flex-1 bg-mobile-screen',
				tabHeight && { marginBottom: BottomTabBarHeight }
			)}
		>
			{children}
		</ScrollView>
	) : (
		<View
			style={twStyle(
				'flex-1 justify-between gap-10 bg-mobile-screen py-6',
				style,
				tabHeight && { marginBottom: BottomTabBarHeight }
			)}
		>
			{children}
		</View>
	);
};

export default ScreenContainer;
