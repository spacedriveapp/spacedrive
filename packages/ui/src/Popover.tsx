import * as Radix from '@radix-ui/react-popover';
import clsx from 'clsx';
import { PropsWithChildren, useState } from 'react';

interface Props extends Radix.PopoverContentProps {
	trigger: React.ReactNode;
	transformOrigin?: string;
	disabled?: boolean;
	className?: string;
}

export const Popover = ({
	trigger,
	children,
	disabled,
	transformOrigin,
	className,
	...props
}: PropsWithChildren<Props>) => {
	const [open, setOpen] = useState(false);

	// const transitions = useTransition(open, {
	// 	from: {
	// 		opacity: 0,
	// 		transform: `scale(0.5)`,
	// 		transformOrigin: transformOrigin || 'top'
	// 	},
	// 	enter: { opacity: 1, transform: 'scale(1)' },
	// 	leave: { opacity: -0.5, transform: 'scale(0.95)' },
	// 	config: { mass: 0.4, tension: 170, friction: 10 }
	// });

	return (
		<Radix.Root open={open} onOpenChange={setOpen}>
			<Radix.Trigger disabled={disabled} asChild>
				{trigger}
			</Radix.Trigger>
			{open && (
				<Radix.Portal forceMount>
					<Radix.Content forceMount asChild>
						<div
							className={clsx(
								'flex flex-col',
								'z-50 m-2 min-w-[11rem] space-y-1',
								'cursor-default select-none rounded-lg',
								'text-ink text-left text-sm',
								'bg-app-overlay ',
								'border-app-line border',
								'shadow-2xl shadow-black/60 ',
								className
							)}
							// style={styles}
						>
							{children}
						</div>
					</Radix.Content>
				</Radix.Portal>
			)}
		</Radix.Root>
	);
};

export { Close as PopoverClose } from '@radix-ui/react-popover';
