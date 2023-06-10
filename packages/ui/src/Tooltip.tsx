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
				<TooltipPrimitive.Portal>
					<TooltipPrimitive.Content
						side={position}
						className="z-50 mb-[2px] max-w-[200px] rounded bg-app-darkBox px-2 py-1 text-center text-xs text-ink"
					>
						<TooltipPrimitive.Arrow className="fill-app-darkBox" />
						{label}
					</TooltipPrimitive.Content>
				</TooltipPrimitive.Portal>
			</TooltipPrimitive.Root>
		</TooltipPrimitive.Provider>
	);
};
