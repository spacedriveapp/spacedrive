import { useNavigation, useRoute } from '@react-navigation/native';
import { ReactNode, useEffect, useRef } from 'react';
import { Platform, View } from 'react-native';
import Animated, { SharedValue, useAnimatedScrollHandler } from 'react-native-reanimated';
import { AnimatedScrollView } from 'react-native-reanimated/lib/typescript/reanimated2/component/ScrollView';
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
	/** Styling of both side fades */
	topFadeStyle?: string;
	bottomFadeStyle?: string;
	scrollY?: SharedValue<number>;
}

const ScreenContainer = ({
	children,
	style,
	topFadeStyle,
	bottomFadeStyle,
	scrollview = true,
	tabHeight = true,
	scrollToBottomOnChange = false,
	scrollY
}: Props) => {
	const ref = useRef<AnimatedScrollView>(null);
	const bottomTabBarHeight = Platform.OS === 'ios' ? 80 : 60;
	const scrollHandler = useAnimatedScrollHandler((e) => {
		if (!scrollY) return;
		scrollY.value = e.contentOffset.y;
	});
	const navigation = useNavigation();
	const route = useRoute();

	//everytime the tab changes we reset the scroll to 0
	useEffect(() => {
		const unsubscribe = navigation.addListener('blur', () => {
			ref.current?.scrollTo({ y: 0, animated: false });
			if (scrollY) scrollY.value = 0;
		});

		return unsubscribe;
	}, [navigation, scrollY]);

	return scrollview ? (
		<View style={tw`relative flex-1`}>
			<Fade
				topFadeStyle={topFadeStyle}
				bottomFadeStyle={bottomFadeStyle}
				screenFade
				fadeSides="top-bottom"
				orientation="vertical"
				color="black"
				width={30}
				height="100%"
			>
				<Animated.ScrollView
					ref={ref}
					onContentSizeChange={() => {
						if (!scrollToBottomOnChange) return;
						ref.current?.scrollToEnd({ animated: true });
					}}
					scrollEventThrottle={1}
					onScroll={scrollHandler}
					contentContainerStyle={twStyle('justify-between gap-10 py-6', style)}
					style={twStyle(
						'flex-1 bg-black',
						tabHeight && { marginBottom: bottomTabBarHeight }
					)}
				>
					{children}
				</Animated.ScrollView>
			</Fade>
		</View>
	) : (
		<View style={tw`relative flex-1`}>
			<Fade
				topFadeStyle={topFadeStyle}
				bottomFadeStyle={bottomFadeStyle}
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
						tabHeight && { marginBottom: bottomTabBarHeight }
					)}
				>
					{children}
				</View>
			</Fade>
		</View>
	);
};

export default ScreenContainer;
