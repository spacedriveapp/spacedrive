import { DimensionValue } from 'react-native';
import LinearGradient from 'react-native-linear-gradient';
import { tw, twStyle } from '~/lib/tailwind';

interface Props {
	children: React.ReactNode; // children of fade
	color: string; // tailwind color of fades - right and left side
	width: DimensionValue; // width of fade
	height: DimensionValue; // height of fade
	style?: string; // tailwind style
	orientation?: 'horizontal' | 'vertical'; // orientation of fade
}

const Fade = ({ children, style, color, width, height, orientation = 'horizontal' }: Props) => {
	return (
		<>
			<LinearGradient
				style={{
					width: orientation === 'vertical' ? height : width,
					height: orientation === 'vertical' ? width : height,
					position: 'absolute',
					top: 0,
					left: 0,
					zIndex: 10,
					...twStyle(style)
				}}
				start={{ x: 0, y: 0 }}
				end={{ x: 1, y: 0 }}
				colors={[tw.color(color) as string, tw.color(color + '/0') as string]}
			/>
			{children}
			<LinearGradient
				style={{
					width: orientation === 'vertical' ? height : width,
					height: orientation === 'vertical' ? width : height,
					position: 'absolute',
					top: 0,
					right: 0,
					zIndex: 10,
					...twStyle(style)
				}}
				start={{ x: 1, y: 0 }}
				end={{ x: 0, y: 0 }}
				colors={[tw.color(color) as string, tw.color(color + '/0') as string]}
			/>
		</>
	);
};

export default Fade;
