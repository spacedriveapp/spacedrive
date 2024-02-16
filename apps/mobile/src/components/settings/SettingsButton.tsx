import { Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

import { Button, ButtonVariants } from '../primitive/Button';

interface Props {
	buttonText: string;
	title: string;
	description?: string;
	buttonPress: () => void;
	buttonVariant?: ButtonVariants;
	buttonTextStyle?: string;
}

const SettingsButton = ({
	buttonText,
	title,
	description,
	buttonVariant,
	buttonTextStyle,
	buttonPress
}: Props) => {
	return (
		<View style={tw`flex-row items-center justify-between`}>
			<View style={tw`w-[75%]`}>
				<Text style={tw`text-sm font-medium text-ink`}>{title}</Text>
				{description && <Text style={tw`mt-1 text-xs text-ink-dull`}>{description}</Text>}
			</View>
			<Button variant={buttonVariant} onPress={buttonPress}>
				<Text style={twStyle(buttonTextStyle)}>{buttonText}</Text>
			</Button>
		</View>
	);
};

export default SettingsButton;
