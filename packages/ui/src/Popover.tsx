import * as Radix from '@radix-ui/react-popover';
import clsx from 'clsx';
import React, { useEffect, useRef, useState } from 'react';

interface Props extends Radix.PopoverContentProps {
	trigger: React.ReactNode;
	disabled?: boolean;
	ignoreOpenState?: boolean; //this makes the PopoverClose component work if set to true
}

export const Popover = ({
	trigger,
	children,
	disabled,
	ignoreOpenState,
	className,
	...props
}: Props) => {
	const [open, setOpen] = useState(false);
	const popOverRef = useRef<HTMLDivElement>(null);
	const triggerRef = useRef<HTMLButtonElement>(null);
	const handleClickOutside = (event: MouseEvent | TouchEvent) => {
		if (popOverRef.current && triggerRef.current) {
			if (
				!popOverRef.current.contains(event.target as Node) &&
				!triggerRef.current.contains(event.target as Node)
			) {
				setOpen(false);
			}
		}
	};
	useEffect(() => {
		const windowResizeListener = () => setOpen(false);
		window.addEventListener('resize', windowResizeListener);
		window.addEventListener('click', handleClickOutside);
		window.addEventListener('touchstart', handleClickOutside);
		return () => {
			window.removeEventListener('resize', windowResizeListener);
			window.removeEventListener('click', handleClickOutside);
			window.removeEventListener('touchstart', handleClickOutside);
		};
	}, []);
	return (
		<Radix.Root open={ignoreOpenState ? undefined : open}>
			<Radix.Trigger
				ref={triggerRef}
				onClick={() => setOpen(!open)}
				disabled={disabled}
				asChild
			>
				{trigger}
			</Radix.Trigger>

			<Radix.Portal>
				<Radix.Content
					ref={popOverRef}
					onOpenAutoFocus={(event) => event.preventDefault()}
					onCloseAutoFocus={(event) => event.preventDefault()}
					className={clsx(
						'flex flex-col',
						'z-50 m-2 min-w-[11rem]',
						'cursor-default select-none rounded-lg',
						'text-left text-sm text-ink',
						'bg-app-overlay',
						'border border-app-line',
						'shadow-2xl',
						'animate-in fade-in',
						className
					)}
					{...props}
				>
					{children}
				</Radix.Content>
			</Radix.Portal>
		</Radix.Root>
	);
};

export { Close as PopoverClose } from '@radix-ui/react-popover';
