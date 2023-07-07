import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import clsx from 'clsx';
import { PropsWithChildren, ReactNode } from 'react';

export interface TooltipProps extends PropsWithChildren {
	label: ReactNode;
	position?: 'top' | 'right' | 'bottom' | 'left';
	className?: string;
	tooltipClassName?: string;
	asChild?: boolean;
}

export const Tooltip = ({ position = 'bottom', ...props }: TooltipProps) => {
	return (
		<TooltipPrimitive.Provider>
			<TooltipPrimitive.Root>
				<TooltipPrimitive.Trigger asChild>
					{props.asChild ? (
						props.children
					) : (
						<span className={props.className}>{props.children}</span>
					)}
				</TooltipPrimitive.Trigger>
				<TooltipPrimitive.Portal>
					{props.label && (
						<TooltipPrimitive.Content
							side={position}
							className={clsx(
								'z-50 max-w-[200px] break-words rounded bg-black px-2 py-1 text-center text-xs text-white',
								props.tooltipClassName
							)}
						>
							<TooltipPrimitive.Arrow className="fill-black" />
							{props.label}
						</TooltipPrimitive.Content>
					)}
				</TooltipPrimitive.Portal>
			</TooltipPrimitive.Root>
		</TooltipPrimitive.Provider>
	);
};
