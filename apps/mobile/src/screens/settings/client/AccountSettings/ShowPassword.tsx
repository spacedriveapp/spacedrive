import { Eye, EyeClosed } from 'phosphor-react-native';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';

interface Props {
	showPassword: boolean;
	setShowPassword: (value: boolean) => void;
}

const ShowPassword = ({ showPassword, setShowPassword }: Props) => {
	return (
		<Button
			variant="gray"
			style={tw`flex size-6 items-center justify-center !p-0`}
			onPressIn={() => setShowPassword(!showPassword)}
		>
			{!showPassword ? <EyeClosed size={12} /> : <Eye size={12} />}
		</Button>
	);
};

export default ShowPassword;
