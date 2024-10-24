import { Eye, EyeClosed } from 'phosphor-react-native';
import { Text } from 'react-native';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';

interface Props {
	showPassword: boolean;
	setShowPassword: (value: boolean) => void;
	plural?: boolean;
}

const ShowPassword = ({ showPassword, setShowPassword, plural }: Props) => {
	return (
		<Button
			variant="gray"
			style={tw`mt-1.5 flex w-full flex-row items-center justify-center gap-2`}
			onPressIn={() => setShowPassword(!showPassword)}
		>
			{!showPassword ? (
				<EyeClosed size={14} color="white" />
			) : (
				<Eye size={14} color="white" />
			)}
			<Text style={tw`font-bold text-ink`}>Show Password{plural ? 's' : ''}</Text>
		</Button>
	);
};

export default ShowPassword;
