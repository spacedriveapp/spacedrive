import * as RadioGroup from '@radix-ui/react-radio-group';
import { cx } from 'class-variance-authority';

import { tw } from './utils';

export const Root = ({ children, ...props }: RadioGroup.RadioGroupProps) => {
	return (
		<RadioGroup.Root {...props}>
			<div className="space-y-3">{children}</div>
		</RadioGroup.Root>
	);
};

// export const Item = tw(
// 	RadioGroup.Item
// )`rounded-md border border-app-line bg-app-box px-4 py-2 flex items-center space-x-2`;

export const Item = ({ children, ...props }: RadioGroup.RadioGroupItemProps) => {
	return (
		<div className="flex max-w-sm px-4 py-3 space-x-2 border rounded-md border-app-line bg-app-box/50">
			<RadioGroup.Item
				id={'radio' + props.value}
				className={cx(
					'peer relative w-4 h-4 rounded-full mr-1 mt-1 flex-shrink-0 border border-transparent',
					'radix-state-checked:bg-accent',
					'radix-state-unchecked:bg-gray-100 dark:radix-state-unchecked:bg-gray-900',
					'focus:outline-none focus:ring-0 focus:ring-offset-0 focus-visible:ring focus-visible:ring-accent focus-visible:ring-opacity-75 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-gray-800'
				)}
				{...props}
			>
				<RadioGroup.Indicator className="absolute inset-0 flex items-center justify-center leading-0">
					<div className="w-1.5 h-1.5 rounded-full bg-white"></div>
				</RadioGroup.Indicator>
			</RadioGroup.Item>
			<label className="" htmlFor={'radio' + props.value}>
				{children}
			</label>
		</div>
	);
};
