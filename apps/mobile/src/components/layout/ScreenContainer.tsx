import { ReactNode, useRef } from 'react';
import { ScrollView, View } from 'react-native';
import { ClassInput } from 'twrnc/dist/esm/types';
import { tw, twStyle } from '~/lib/tailwind';

import Fade from './Fade';

interface Props {
	children: ReactNode;
	/** If true, the container will be a ScrollView */
	scrollview?: boolean;
	style?: ClassInput;
	/** If true, the bottom tab bar height will be added to the bottom of the container */
	tabHeight?: boolean;
	scrollToBottomOnChange?: boolean;
}

const BottomTabBarHeight = 80;

const ScreenContainer = ({
	children,
	style,
	scrollview = true,
	tabHeight = true,
	scrollToBottomOnChange = false
}: Props) => {
	const ref = useRef<ScrollView>(null);
	return scrollview ? (
		<View style={tw`relative flex-1`}>
			<Fade
				screenFade
				fadeSides="top-bottom"
				orientation="vertical"
				color="black"
				width={30}
				height="100%"
			>
				<ScrollView
					ref={ref}
					onContentSizeChange={() => {
						if (!scrollToBottomOnChange) return;
						ref.current?.scrollToEnd({ animated: true });
					}}
					contentContainerStyle={twStyle('justify-between gap-10 py-6', style)}
					style={twStyle(
						'flex-1 bg-black',
						tabHeight && { marginBottom: BottomTabBarHeight }
					)}
				>
					{children}
				</ScrollView>
			</Fade>
		</View>
	) : (
		<View style={tw`relative flex-1`}>
			<Fade
				screenFade
				fadeSides="top-bottom"
				orientation="vertical"
				color="black"
				width={30}
				height="100%"
			>
				<View
					style={twStyle(
						'flex-1 justify-between gap-10 bg-black py-6',
						style,
						tabHeight && { marginBottom: BottomTabBarHeight }
					)}
				>
					{children}
				</View>
			</Fade>
		</View>
	);
};

export default ScreenContainer;
