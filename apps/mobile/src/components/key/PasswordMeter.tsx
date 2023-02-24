import { Text, View } from 'react-native';
import { getPasswordStrength } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

// NOTE: Lazy load this component
export const PasswordMeter = (props: { password: string }) => {
	const { score, scoreText } = getPasswordStrength(props.password);

	return (
		<View style={tw`relative`}>
			<Text style={tw`text-sm`}>Password strength</Text>
			<Text
				style={twStyle(
					'absolute top-0.5 right-0 px-1 text-sm font-semibold',
					score === 0 && 'text-red-500',
					score === 1 && 'text-red-500',
					score === 2 && 'text-amber-400',
					score === 3 && 'text-lime-500',
					score === 4 && 'text-accent'
				)}
			>
				{scoreText}
			</Text>
			<View style={tw`flex grow`}>
				<View style={tw`bg-app-box/50 mt-2 w-full rounded-full`}>
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
		</View>
	);
};
