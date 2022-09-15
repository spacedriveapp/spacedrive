import * as ProgressPrimitive from '@radix-ui/react-progress';

interface Props {
	value: number;
	total: number;
}

const ProgressBar = (props: Props) => {
	const percentage = Math.round((props.value / props.total) * 100);
	return (
		<ProgressPrimitive.Root
			value={percentage}
			className="w-full h-1 overflow-hidden bg-gray-200 rounded-full dark:bg-gray-500"
		>
			<ProgressPrimitive.Indicator
				style={{ width: `${percentage}%` }}
				className="h-full duration-300 ease-in-out bg-primary "
			/>
		</ProgressPrimitive.Root>
	);
};

export default ProgressBar;
