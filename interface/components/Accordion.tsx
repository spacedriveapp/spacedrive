import clsx from 'clsx';
import { CaretDown } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';

interface Props {
	className?: string;
	title: string;
	titleClassName?: string;
	containerClassName?: string;
	caretSize?: number;
}

const Accordion = (props: PropsWithChildren<Props>) => {
	const [toggle, setToggle] = useState(false);

	return (
		<div className={clsx(props.className, 'rounded-md border border-app-line bg-app-darkBox')}>
			<div
				onClick={() => setToggle((t) => !t)}
				className={clsx(
					'flex flex-row items-center justify-between px-3 py-2',
					props.titleClassName
				)}
			>
				<p className="text-xs">{props.title}</p>
				<CaretDown
					size={props.caretSize || 12}
					className={clsx(toggle && 'rotate-180', 'transition-all duration-200')}
				/>
			</div>
			{toggle && (
				<div
					className={clsx(
						'rounded-b-md border-t border-app-line bg-app-box p-3 py-2',
						props.containerClassName
					)}
				>
					{props.children}
				</div>
			)}
		</div>
	);
};

export default Accordion;
