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
			className="h-1 w-[94%] overflow-hidden rounded-full bg-app-button"
		>
			<ProgressPrimitive.Indicator
				style={{ width: `${percentage}%` }}
				className="h-full bg-accent duration-300 ease-in-out "
			/>
		</ProgressPrimitive.Root>
	);
});
