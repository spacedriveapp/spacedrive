import * as TooltipPrimitive from '@radix-ui/react-tooltip';
import clsx from 'clsx';
import { PropsWithChildren, ReactNode } from 'react';

import { ModifierKeys } from './keys';

export interface TooltipProps extends PropsWithChildren {
	label: ReactNode;
	position?: 'top' | 'right' | 'bottom' | 'left';
	className?: string;
	tooltipClassName?: string;
	asChild?: boolean;
	hoverable?: boolean;
	keybinds?: Array<String | ModifierKeys>;
}

const separateKeybinds = (keybinds: TooltipProps['keybinds']): TooltipProps['keybinds'] => {
	if (!keybinds) return;
	const arr = [];
	for (const i of keybinds) {
		for (const j of i) {
			arr.push(j);
		}
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
								'TooltipContent z-50 mt-1 flex max-w-[200px] select-text items-center gap-2 break-words rounded border border-app-line bg-app-box px-2 py-1 text-center text-xs text-ink',
								props.tooltipClassName
							)}
						>
							{props.label}
							{props.keybinds && (
								<div className="flex items-center justify-center gap-1">
									{separateKeybinds(props.keybinds)?.map((k, _) => (
										<kbd
											key={k}
											className={
												'h-4.5 flex items-center justify-center rounded-md border border-app-selected bg-app-selected/50 px-1.5 py-0.5 text-[10px] text-ink'
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
