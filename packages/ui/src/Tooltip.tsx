import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import clsx from 'clsx';
import { PropsWithChildren, ReactNode } from 'react';

export interface TooltipProps extends PropsWithChildren {
	label: ReactNode;
	position?: 'top' | 'right' | 'bottom' | 'left';
	className?: string;
	tooltipClassName?: string;
	asChild?: boolean;
	hoverable?: boolean;
	keybinds?: string;
}

const stringToArray = (word: string): string[] | undefined => {
	if (!word) return;
	const arr: string[] = [];
	for (const k of word) {
		arr.push(k);
	}
	return arr;
};

export const Tooltip = ({ position = 'bottom', hoverable = true, ...props }: TooltipProps) => {
	return (
		<TooltipPrimitive.Provider disableHoverableContent={!hoverable}>
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
								'TooltipContent z-50 mt-1 flex max-w-[200px] select-text items-center gap-2 break-words rounded border border-app-lightBox/40 bg-app-input px-2 py-1 text-center text-xs text-white',
								props.tooltipClassName
							)}
						>
							{props.label}
							{props.keybinds && (
								<div className="mx-auto flex w-fit justify-center gap-1">
									{stringToArray(props.keybinds)?.map((k, _) => (
										<kbd
											key={k}
											className={
												'rounded-md border border-app-lightBox/60 bg-app-lightBox/40 px-1.5 py-1 text-[10px] text-white'
											}
										>
											<p>{k}</p>
										</kbd>
									))}
								</div>
							)}
						</TooltipPrimitive.Content>
					)}
				</TooltipPrimitive.Portal>
			</TooltipPrimitive.Root>
		</TooltipPrimitive.Provider>
	);
};
