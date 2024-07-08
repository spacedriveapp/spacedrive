import { ReactNode, useRef } from 'react';
import { Platform, ScrollView, View } from 'react-native';
import { ClassInput } from 'twrnc/dist/esm/types';
import { tw, twStyle } from '~/lib/tailwind';

interface Props {
	children: ReactNode;
	/** If true, the container will be a ScrollView */
	scrollview?: boolean;
	style?: ClassInput;
	/** If true, the bottom tab bar height will be added to the bottom of the container */
	tabHeight?: boolean;
	scrollToBottomOnChange?: boolean;
}

const ScreenContainer = ({
	children,
	style,
	scrollview = true,
	tabHeight = true,
	scrollToBottomOnChange = false
}: Props) => {
	const ref = useRef<ScrollView>(null);
	const bottomTabBarHeight = Platform.OS === 'ios' ? 80 : 60;
	return scrollview ? (
		<View style={tw`relative flex-1`}>
			<ScrollView
				ref={ref}
				onContentSizeChange={() => {
					if (!scrollToBottomOnChange) return;
					ref.current?.scrollToEnd({ animated: true });
				}}
				contentContainerStyle={twStyle('justify-between gap-10 py-6', style)}
				style={twStyle('bg-black', tabHeight && { marginBottom: bottomTabBarHeight })}
			>
				{children}
			</ScrollView>
		</View>
	) : (
		<View style={tw`relative flex-1`}>
			<View
				style={twStyle(
					'flex-1 justify-between gap-10 bg-black py-6',
					style,
					tabHeight && { marginBottom: bottomTabBarHeight }
				)}
			>
				{children}
			</View>
		</View>
	);
};

export default ScreenContainer;
