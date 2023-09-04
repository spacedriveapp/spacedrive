import clsx from 'clsx';
import { CaretDown } from 'phosphor-react';
import { useState } from 'react';

interface Props {
	children: React.ReactNode;
	className?: string;
	title: string;
}

const Accordion = ({ title, className, children }: Props) => {
	const [toggle, setToggle] = useState(false);

	return (
		<div className={clsx(className, 'rounded-md border border-app-line bg-app-darkBox')}>
			<div
				onClick={() => setToggle((t) => !t)}
				className="flex items-center justify-between px-3 py-2"
			>
				<p className="text-xs">{title}</p>
				<CaretDown
					className={clsx(toggle && 'rotate-180', 'transition-all duration-200')}
				/>
			</div>
			{toggle && (
				<div className="rounded-b-md border-t border-app-line bg-app-box p-3 pt-2">
					{children}
				</div>
			)}
		</div>
	);
};

export default Accordion;
