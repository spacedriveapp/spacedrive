import clsx from 'clsx';
import { CaretDown } from 'phosphor-react';
import { useState } from 'react';

interface Props {
	children: React.ReactNode;
	className?: string;
	title: string;
	titleClassName?: string;
	containerClassName?: string;
	caretSize?: number;
}

const Accordion = ({
	title,
	className,
	children,
	titleClassName,
	containerClassName,
	caretSize
}: Props) => {
	const [toggle, setToggle] = useState(false);

	return (
		<div className={clsx(className, 'rounded-md border border-app-line bg-app-darkBox')}>
			<div
				onClick={() => setToggle((t) => !t)}
				className={clsx(
					'flex flex-row items-center justify-between px-3 py-2',
					titleClassName
				)}
			>
				<p className="text-xs">{title}</p>
				<CaretDown
					size={caretSize || 12}
					className={clsx(toggle && 'rotate-180', 'transition-all duration-200')}
				/>
			</div>
			{toggle && (
				<div
					className={clsx(
						'rounded-b-md border-t border-app-line bg-app-box p-3 py-2',
						containerClassName
					)}
				>
					{children}
				</div>
			)}
		</div>
	);
};

export default Accordion;
