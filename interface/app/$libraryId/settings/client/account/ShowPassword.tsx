import { Eye, EyeClosed } from '@phosphor-icons/react';
import { Button, Tooltip } from '@sd/ui';

interface Props {
	showPassword: boolean;
	setShowPassword: (value: boolean) => void;
}

const ShowPassword = ({ showPassword, setShowPassword }: Props) => {
	return (
		<Tooltip
			className="absolute inset-y-0 right-1 flex items-center"
			position="top"
			label="Show password"
		>
			<Button
				variant="gray"
				className="flex size-6 items-center justify-center !p-0"
				onClick={() => setShowPassword(!showPassword)}
			>
				{!showPassword ? <EyeClosed size={12} /> : <Eye size={12} />}
			</Button>
		</Tooltip>
	);
};

export default ShowPassword;
