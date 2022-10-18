import { MotiView, useDynamicAnimation } from 'moti';
import { ReactNode } from 'react';
import { StyleSheet, View } from 'react-native';
import { useDerivedValue, useSharedValue } from 'react-native-reanimated';
import Layout from '~/constants/Layout';
import tw from '~/lib/tailwind';

// Anything wrapped with FadeIn will fade in on mount.
export const FadeInAnimation = ({ children, delay }: { children: any; delay?: number }) => (
	<MotiView from={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ type: 'timing', delay }}>
		{children}
	</MotiView>
);

export const FadeInUpAnimation = ({ children, delay }: { children: any; delay?: number }) => (
	<MotiView
		from={{ opacity: 0, translateY: 20 }}
		animate={{ opacity: 1, translateY: 0 }}
		transition={{ type: 'timing', delay }}
	>
		{children}
	</MotiView>
);

export const LogoAnimation = ({ children }: { children: any }) => (
	<MotiView
		from={{ opacity: 0.8, translateY: Layout.window.width / 2 }}
		animate={{ opacity: 1, translateY: 0 }}
		transition={{ type: 'timing', delay: 200 }}
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
} & React.ComponentProps<typeof MotiView>;

export function AnimatedHeight({
	children,
	hide = false,
	style,
	delay = 0,
	transition = { type: 'timing', delay },
	onHeightDidAnimate,
	initialHeight = 0,
	...motiViewProps
}: AnimatedHeightProps) {
	const measuredHeight = useSharedValue(initialHeight);
	const state = useDynamicAnimation(() => {
		return {
			height: initialHeight,
			opacity: !initialHeight || hide ? 0 : 1
		};
	});
	if ('state' in motiViewProps) {
		console.warn('[AnimateHeight] state prop not supported');
	}

	useDerivedValue(() => {
		let height = Math.ceil(measuredHeight.value);
		if (hide) {
			height = 0;
		}

		state.animateTo({
			height,
			opacity: !height || hide ? 0 : 1
		});
	}, [hide, measuredHeight]);

	return (
		<MotiView
			{...motiViewProps}
			state={state}
			transition={transition}
			onDidAnimate={
				onHeightDidAnimate &&
				((key, finished, _, { attemptedValue }) =>
					key === 'height' && onHeightDidAnimate(attemptedValue as number))
			}
			style={[tw`overflow-hidden`, style]}
		>
			<View
				style={[StyleSheet.absoluteFill, { bottom: 'auto' }]}
				onLayout={({ nativeEvent }) => {
					measuredHeight.value = nativeEvent.layout.height;
				}}
			>
				{children}
			</View>
		</MotiView>
	);
}
