import * as DP from '@radix-ui/react-popover';
import clsx from 'clsx';
import { PropsWithChildren, ReactNode } from 'react';

interface Props extends PropsWithChildren, DP.PopoverContentProps {
	trigger: ReactNode;
}

export const Popover = ({ trigger, children, className, ...props }: Props) => {
	return (
		<DP.Root>
			<DP.Trigger asChild>{trigger}</DP.Trigger>
			<DP.Portal>
				<DP.Content
					align="center"
					sideOffset={4}
					collisionPadding={10}
					className={clsx(
						'rounded-lg text-sm text-ink select-none cursor-default bg-app-overlay border border-app-line shadow-2xl shadow-black/60',
						className
					)}
					{...props}
				>
					{children}
				</DP.Content>
			</DP.Portal>
		</DP.Root>
	);
};

export * from '@radix-ui/react-popover';
