import { MotiView } from 'moti';
import { PropsWithChildren, ReactNode } from 'react';
import { StyleSheet, ViewProps } from 'react-native';
import Animated, {
	runOnJS,
	useAnimatedStyle,
	useSharedValue,
	withTiming
} from 'react-native-reanimated';
import Layout from '~/constants/Layout';

type MotiViewProps = PropsWithChildren<ViewProps>;

// Anything wrapped with FadeIn will fade in on mount.
export const FadeInAnimation = ({
	children,
	delay,
	...props
}: MotiViewProps & { delay?: number }) => (
	<MotiView
		from={{ opacity: 0 }}
		animate={{ opacity: 1 }}
		transition={{ type: 'timing', delay }}
		{...props}
	>
		{children}
	</MotiView>
);

export const FadeInUpAnimation = ({
	children,
	delay,
	...props
}: MotiViewProps & { delay?: number }) => (
	<MotiView
		from={{ opacity: 0, translateY: 20 }}
		animate={{ opacity: 1, translateY: 0 }}
		transition={{ type: 'timing', delay }}
		{...props}
	>
		{children}
	</MotiView>
);

export const LogoAnimation = ({ children, ...props }: MotiViewProps) => (
	<MotiView
		transition={{ type: 'timing', delay: 200 }}
		from={{ opacity: 0.8, translateY: Layout.window.width / 2 }}
		animate={{ opacity: 1, translateY: 0 }}
		{...props}
	>
		{children}
	</MotiView>
);

type AnimatedHeightProps = {
	children?: ReactNode;
	/**
	 * If `true`, the height will automatically animate to 0. Default: `false`.
	 */
	hide?: boolean;
	onHeightDidAnimate?: (height: number) => void;
	initialHeight?: number;
	duration?: number;
} & MotiViewProps;

export function AnimatedHeight({
	children,
	hide = !children,
	style,
	onHeightDidAnimate,
	duration = 200,
	initialHeight = 0
}: AnimatedHeightProps) {
	const measuredHeight = useSharedValue(initialHeight);
	const childStyle = useAnimatedStyle(
		() => ({
			opacity: withTiming(!measuredHeight.value || hide ? 0 : 1, { duration })
		}),
		[hide, measuredHeight]
	);

	const containerStyle = useAnimatedStyle(() => {
		return {
			height: withTiming(hide ? 0 : measuredHeight.value, { duration }, () => {
				if (onHeightDidAnimate) {
					runOnJS(onHeightDidAnimate)(measuredHeight.value);
				}
			})
		};
	}, [hide, measuredHeight]);

	return (
		<Animated.View style={[styles.hidden, style, containerStyle]}>
			<Animated.View
				style={[StyleSheet.absoluteFill, styles.autoBottom, childStyle]}
				onLayout={({ nativeEvent }) => {
					measuredHeight.value = Math.ceil(nativeEvent.layout.height);
				}}
			>
				{children}
			</Animated.View>
		</Animated.View>
	);
}

const styles = StyleSheet.create({
	autoBottom: {
		bottom: 'auto'
	},
	hidden: {
		overflow: 'hidden'
	}
});
