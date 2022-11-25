import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import { PropsWithChildren } from 'react';

export interface TooltipProps {
	label: string;
	position?: 'top' | 'right' | 'bottom' | 'left';
	className?: string;
}

export const Tooltip = ({
	children,
	label,
	position = 'bottom',
	className
}: PropsWithChildren<TooltipProps>) => {
	return (
		<TooltipPrimitive.Provider>
			<TooltipPrimitive.Root>
				<TooltipPrimitive.Trigger asChild>
					<span className={className}>{children}</span>
				</TooltipPrimitive.Trigger>
				<TooltipPrimitive.Content
					side={position}
					className="text-xs max-w-[200px] z-50 text-center rounded px-2 py-1 mb-[2px] bg-gray-300 dark:!bg-gray-900 dark:text-gray-100"
				>
					<TooltipPrimitive.Arrow className="fill-gray-300 dark:!fill-gray-900" />
					{label}
				</TooltipPrimitive.Content>
			</TooltipPrimitive.Root>
		</TooltipPrimitive.Provider>
	);
};
