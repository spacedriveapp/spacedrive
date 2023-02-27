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
			className="h-1 w-full overflow-hidden rounded-full bg-gray-200 dark:bg-gray-500"
		>
			<ProgressPrimitive.Indicator
				style={{ width: `${percentage}%` }}
				className="bg-accent h-full duration-300 ease-in-out "
			/>
		</ProgressPrimitive.Root>
	);
};

export default ProgressBar;
