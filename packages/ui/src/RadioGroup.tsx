import * as RadioGroup from '@radix-ui/react-radio-group';
import { cx } from 'class-variance-authority';
import clsx from 'clsx';
import { forwardRef } from 'react';

export interface RootProps extends RadioGroup.RadioGroupProps {}
export const Root = forwardRef<HTMLDivElement, RootProps>(
	({ children, className, ...props }, ref) => {
		return (
			<RadioGroup.Root {...props} ref={ref}>
				<div className={clsx('space-y-3', className)}>{children}</div>
			</RadioGroup.Root>
		);
	}
);

// export const Item = tw(
// 	RadioGroup.Item
// )`rounded-md border border-app-line bg-app-box px-4 py-2 flex items-center space-x-2`;

export interface ItemProps extends RadioGroup.RadioGroupItemProps {}
export const Item = ({ children, ...props }: ItemProps) => {
	return (
		<div className="border-app-line bg-app-box/50 flex max-w-sm space-x-2 rounded-md border px-4 py-3">
			<RadioGroup.Item
				id={'radio' + props.value}
				className={cx(
					'peer relative mr-1 mt-1 h-4 w-4 flex-shrink-0 rounded-full border border-transparent',
					'radix-state-checked:bg-accent',
					'radix-state-unchecked:bg-gray-100 dark:radix-state-unchecked:bg-gray-900',
					'focus-visible:ring-accent focus:outline-none focus:ring-0 focus:ring-offset-0 focus-visible:ring focus-visible:ring-opacity-75 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-gray-800'
				)}
				{...props}
			>
				<RadioGroup.Indicator className="leading-0 absolute inset-0 flex items-center justify-center">
					<div className="h-1.5 w-1.5 rounded-full bg-white"></div>
				</RadioGroup.Indicator>
			</RadioGroup.Item>
			<label className="" htmlFor={'radio' + props.value}>
				{children}
			</label>
		</div>
	);
};
