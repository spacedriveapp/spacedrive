import { View } from 'react-native';
import { ClassInput } from 'twrnc';
import { tw, twStyle } from '~/lib/tailwind';

//border-style is not supported - so this is a way to do it

interface Props {
	color?: string;
	dotCount?: number;
	style?: ClassInput;
}

const DottedDivider = ({ dotCount = 100, color = 'bg-app-lightborder', style }: Props) => {
	return (
		<View style={tw`flex-1 flex-row items-center gap-0.5 overflow-hidden`}>
			{Array.from({ length: dotCount }).map((_, index) => (
				<View
					key={index}
					style={twStyle(`h-0.5 w-0.5 rounded-full`, style, {
						backgroundColor: tw.color(color)
					})}
				/>
			))}
		</View>
	);
};

export default DottedDivider;
