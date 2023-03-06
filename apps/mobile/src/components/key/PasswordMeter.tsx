import { Text, View, ViewStyle } from 'react-native';
import { getPasswordStrength } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

// NOTE: Lazy load this component.

type PasswordMeterProps = {
	password: string;
	containerStyle?: ViewStyle;
};

const PasswordMeter = (props: PasswordMeterProps) => {
	const { score, scoreText } = getPasswordStrength(props.password);

	return (
		<View style={props.containerStyle}>
			<View style={tw`flex flex-row items-center justify-between`}>
				<Text style={tw`text-sm text-white`}>Password strength</Text>
				<Text
					style={twStyle(
						'text-sm font-semibold',
						score === 0 && 'text-red-500',
						score === 1 && 'text-red-500',
						score === 2 && 'text-amber-400',
						score === 3 && 'text-lime-500',
						score === 4 && 'text-accent'
					)}
				>
					{scoreText}
				</Text>
			</View>
			<View style={tw`bg-app-box/80 mt-2 w-full rounded-full`}>
				<View
					style={twStyle(
						{
							width: `${score !== 0 ? score * 25 : 12.5}%`
						},
						'h-2 rounded-full',
						score === 0 && 'bg-red-500',
						score === 1 && 'bg-red-500',
						score === 2 && 'bg-amber-400',
						score === 3 && 'bg-lime-500',
						score === 4 && 'bg-accent'
					)}
				/>
			</View>
		</View>
	);
};

export default PasswordMeter;
