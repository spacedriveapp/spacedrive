import * as ProgressPrimitive from '@radix-ui/react-progress';
import { memo } from 'react';

export interface ProgressBarProps {
	value: number;
	total: number;
}

export const ProgressBar = memo((props: ProgressBarProps) => {
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
});
