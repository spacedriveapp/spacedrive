import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import clsx from 'clsx';
import { PropsWithChildren } from 'react';

export interface TooltipProps {
	label: string;
	position?: 'top' | 'right' | 'bottom' | 'left';
	className?: string;
	tooltipClassName?: string;
}

export const Tooltip = ({
	children,
	label,
	position = 'bottom',
	className,
	tooltipClassName
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
						className={clsx(
							'z-50 mb-[2px] max-w-[200px] break-words rounded bg-app-darkBox px-2 py-1 text-center text-xs text-ink',
							tooltipClassName
						)}
					>
						<TooltipPrimitive.Arrow className="fill-app-darkBox" />
						{label}
					</TooltipPrimitive.Content>
				</TooltipPrimitive.Portal>
			</TooltipPrimitive.Root>
		</TooltipPrimitive.Provider>
	);
};
