import * as Radix from '@radix-ui/react-popover';
import clsx from 'clsx';

interface Props extends Radix.PopoverContentProps {
	trigger: React.ReactNode;
	disabled?: boolean;
}

export const Popover = ({ trigger, children, disabled, className, ...props }: Props) => {
	return (
		<Radix.Root>
			<Radix.Trigger disabled={disabled} asChild>
				{trigger}
			</Radix.Trigger>

			<Radix.Portal>
				<Radix.Content
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
