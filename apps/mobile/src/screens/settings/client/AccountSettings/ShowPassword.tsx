import { Eye, EyeClosed } from 'phosphor-react-native';
import { Text, View } from 'react-native';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';

interface Props {
	showPassword: boolean;
	setShowPassword: (value: boolean) => void;
	plural?: boolean;
}

const ShowPassword = ({ showPassword, setShowPassword, plural }: Props) => {
	return (
		<View style={tw`pt-2`}>
			<Button
				variant="gray"
				style={tw`flex size-6 flex-row items-center justify-center gap-2 !p-0`}
				onPressIn={() => setShowPassword(!showPassword)}
			>
				{!showPassword ? (
					<EyeClosed size={12} color="white" />
				) : (
					<Eye size={12} color="white" />
				)}
				<Text style={tw`text-ink`}>Show Password{plural ? 's' : ''}</Text>
			</Button>
		</View>
	);
};

export default ShowPassword;
