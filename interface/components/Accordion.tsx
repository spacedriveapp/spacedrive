import { CaretDown } from '@phosphor-icons/react';
import clsx from 'clsx';
import { PropsWithChildren, useState } from 'react';

interface Props {
	caretSize?: number;
	title: string;
	variant?: keyof typeof styles;
	className?: string;
	isOpen?: boolean;
	onToggle?: (isOpen: boolean) => void;
}

const styles = {
	default: {
		container: 'flex flex-col gap-1 rounded-b-none px-3 py-2 bg-app-box',
		title: 'flex flex-row items-center justify-between px-3 py-2',
		box: 'rounded-md border border-app-line bg-app-darkBox'
	},
	apple: {
		container: 'flex flex-col gap-1 rounded-b-none px-4',
		title: 'flex flex-row-reverse items-center justify-end gap-2 px-4 pb-1 pt-0 text-ink-dull',
		box: 'rounded-none border-0 bg-transparent py-0'
	}
};

export const Accordion = ({ isOpen = false, ...props }: PropsWithChildren<Props>) => {
	const [toggle, setToggle] = useState(isOpen);
	const variant = styles[props.variant ?? 'default'];
	return (
		<div className={clsx(variant.box, props.className)}>
			<div
				onClick={() => {
					setToggle((t) => !t);
					props.onToggle?.(!toggle);
				}}
				className={variant.title}
			>
				<p className="text-xs">{props.title}</p>
				<CaretDown
					size={props.caretSize || 12}
					className={clsx(
						(isOpen || toggle) && 'rotate-180',
						'transition-all duration-200'
					)}
				/>
			</div>
			{(isOpen || toggle) && <div className={variant.container}>{props.children}</div>}
		</div>
	);
};
