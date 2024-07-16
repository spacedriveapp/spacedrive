import { Text, View } from 'react-native';
import { ClassInput } from 'twrnc/dist/esm/types';
import { tw, twStyle } from '~/lib/tailwind';

import { Button, ButtonVariants } from '../primitive/Button';

interface Props {
	buttonText: string;
	title: string;
	description?: string;
	onPress?: () => void;
	buttonVariant?: ButtonVariants;
	buttonTextStyle?: string;
	buttonIcon?: JSX.Element;
	infoContainerStyle?: ClassInput;
}

const SettingsButton = ({
	buttonText,
	title,
	description,
	buttonVariant,
	buttonTextStyle,
	buttonIcon,
	infoContainerStyle,
	onPress
}: Props) => {
	return (
		<View style={tw`flex-row items-center justify-between`}>
			<View style={twStyle('w-73%', infoContainerStyle)}>
				<Text style={tw`text-sm font-medium text-ink`}>{title}</Text>
				{description && <Text style={tw`mt-1 text-xs text-ink-dull`}>{description}</Text>}
			</View>
			<Button
				style={tw`flex-row items-center gap-2`}
				variant={buttonVariant}
				onPress={onPress}
			>
				{buttonIcon}
				<Text style={twStyle(buttonTextStyle)}>{buttonText}</Text>
			</Button>
		</View>
	);
};

export default SettingsButton;
