'use client';

import * as ProgressPrimitive from '@radix-ui/react-progress';
import clsx from 'clsx';
import { memo } from 'react';

export type ProgressBarProps = {
	pending?: boolean;
} & (
	| {
			value: number;
			total: number;
	  }
	| {
			percent: number;
	  }
);

export const ProgressBar = memo((props: ProgressBarProps) => {
	const percentage = props.pending
		? 0
		: 'percent' in props
			? props.percent
			: Math.round((props.value / props.total) * 100);

	if (props.pending) {
		return (
			<div className="indeterminate-progress-bar h-1 bg-app-button">
				<div className="indeterminate-progress-bar__progress bg-accent"></div>
			</div>
		);
	}
	return (
		<ProgressPrimitive.Root
			value={percentage}
			className={clsx('h-1 w-[94%] overflow-hidden rounded-full bg-app-button')}
		>
			<ProgressPrimitive.Indicator
				style={{ width: `${percentage}%` }}
				className={clsx('h-full bg-accent duration-500 ease-in-out')}
			/>
		</ProgressPrimitive.Root>
	);
});
