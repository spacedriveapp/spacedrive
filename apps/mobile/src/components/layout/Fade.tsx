import { DimensionValue, Platform } from 'react-native';
import LinearGradient from 'react-native-linear-gradient';
import { ClassInput } from 'twrnc';
import { tw, twStyle } from '~/lib/tailwind';

interface Props {
	children: React.ReactNode; // children of fade
	color: string; // tailwind color of fades - right and left side
	width: DimensionValue; // width of fade
	height: DimensionValue; // height of fade
	orientation?: 'horizontal' | 'vertical'; // orientation of fade
	fadeSides?: 'left-right' | 'top-bottom'; // which sides to fade
	screenFade?: boolean; // if true, the fade will consider the bottom tab bar height
	bottomFadeStyle?: ClassInput; // tailwind style for bottom fade
	topFadeStyle?: ClassInput; // tailwind style for top fade
}

const Fade = ({
	children,
	color,
	width,
	height,
	bottomFadeStyle,
	topFadeStyle,
	screenFade = false,
	fadeSides = 'left-right',
	orientation = 'horizontal'
}: Props) => {
	const bottomTabBarHeight = Platform.OS === 'ios' ? 80 : 60;
	const gradientStartEndMap = {
		'left-right': { start: { x: 0, y: 0 }, end: { x: 1, y: 0 } },
		'top-bottom': { start: { x: 0, y: 1 }, end: { x: 0, y: 0 } }
	};
	return (
		<>
			<LinearGradient
				style={{
					width: orientation === 'vertical' ? height : width,
					height: orientation === 'vertical' ? width : height,
					position: 'absolute',
					top: 0,
					alignSelf: 'center',
					left: fadeSides === 'left-right' ? 0 : undefined,
					transform: fadeSides === 'left-right' ? undefined : [{ rotate: '180deg' }],
					zIndex: 10,
					...twStyle(topFadeStyle)
				}}
				start={gradientStartEndMap[fadeSides].start}
				end={gradientStartEndMap[fadeSides].end}
				colors={[tw.color(color) as string, tw.color(color + '/0') as string]}
			/>
			{children}
			<LinearGradient
				style={{
					width: orientation === 'vertical' ? height : width,
					height: orientation === 'vertical' ? width : height,
					position: 'absolute',
					alignSelf: 'center',
					top: fadeSides === 'left-right' ? 0 : undefined,
					bottom:
						fadeSides === 'top-bottom' && !screenFade
							? 0
							: screenFade
								? bottomTabBarHeight
								: 0, // bottom tab bar height
					right: fadeSides === 'left-right' ? 0 : undefined,
					transform: fadeSides === 'top-bottom' ? undefined : [{ rotate: '180deg' }],
					zIndex: 10,
					...twStyle(bottomFadeStyle)
				}}
				start={gradientStartEndMap[fadeSides].start}
				end={gradientStartEndMap[fadeSides].end}
				colors={[tw.color(color) as string, tw.color(color + '/0') as string]}
			/>
		</>
	);
};

export default Fade;
