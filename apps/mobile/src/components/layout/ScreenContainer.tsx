import { useNavigation } from '@react-navigation/native';
import { ReactNode, useEffect, useRef } from 'react';
import { Platform, View } from 'react-native';
import Animated, { SharedValue, useAnimatedScrollHandler } from 'react-native-reanimated';
import { AnimatedScrollView } from 'react-native-reanimated/lib/typescript/reanimated2/component/ScrollView';
import { ClassInput } from 'twrnc/dist/esm/types';
import { tw, twStyle } from '~/lib/tailwind';

import Header, { HeaderProps } from '../header/Header';

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
	/* Header properties */
	header?: HeaderProps;
	hideHeader?: boolean; // Hide the header
}

const ScreenContainer = ({
	children,
	style,
	header,
	scrollY,
	hideHeader = false,
	scrollview = true,
	tabHeight = true,
	scrollToBottomOnChange = false,
}: Props) => {
	const ref = useRef<AnimatedScrollView>(null);
	const bottomTabBarHeight = Platform.OS === 'ios' ? 80 : 60;
	const scrollHandler = useAnimatedScrollHandler((e) => {
		if (scrollY) scrollY.value = e.contentOffset.y;
	});

	const navigation = useNavigation();

// Reset scroll position to 0 whenever the tab blurs or focuses
useEffect(() => {
    const resetScroll = () => {
        ref.current?.scrollTo({ y: 0, animated: false });
        if (scrollY) scrollY.value = 0;
    };

    // Subscribe to blur and focus events
    const unsubscribeBlur = navigation.addListener('blur', resetScroll);
    const unsubscribeFocus = navigation.addListener('focus', resetScroll);

    // Cleanup function to remove event listeners
    return () => {
        unsubscribeBlur();
        unsubscribeFocus();
    };
}, [navigation, scrollY]);


	return scrollview ? (
		<View style={tw`relative flex-1`}>
			{!hideHeader && <Header {...header} scrollY={scrollY} />}
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
		</View>
	) : (
		<View style={tw`relative flex-1`}>
			{!hideHeader && <Header {...header} />}
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
