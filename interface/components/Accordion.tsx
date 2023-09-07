import clsx from 'clsx';
import { CaretDown } from 'phosphor-react';
import { PropsWithChildren, useState } from 'react';


type variants = 'apple' | 'default';

type Styles = {
	container: Record<variants, string>;
	title: Record<variants, string>;
	box: Record<variants, string>;
  };

interface Props {
	caretSize?: number;
	title: string;
	titleVariant?: variants;
	boxVariant?: variants;
	containerVariant?: variants;
}

const styles: Styles = {
			container: {
				apple: 'flex flex-col gap-1 rounded-b-none px-4',
				default: 'rounded-b-md border-t border-app-line bg-app-box p-3 py-2'
			},
			title: {
				apple: 'flex flex-row-reverse items-center justify-end gap-2 px-4 pb-1 pt-0 text-ink-dull',
				default: 'flex flex-row items-center justify-between px-3 py-2'
			},
			box: {
				apple: 'rounded-none border-0 bg-transparent py-0',
				default: 'rounded-md border border-app-line bg-app-darkBox'
			},
	}

const Accordion = (props: PropsWithChildren<Props>) => {
	const [toggle, setToggle] = useState(false);

	return (
		<div className={styles.box[props.boxVariant ?? 'default']}>
			<div
				onClick={() => setToggle((t) => !t)}
				className={styles.title[props.titleVariant ?? 'default']}
			>
				<p className="text-xs">{props.title}</p>
				<CaretDown
					size={props.caretSize || 12}
					className={clsx(toggle && 'rotate-180', 'transition-all duration-200')}
				/>
			</div>
			{toggle && (
				<div
					className={styles.container[props.containerVariant ?? 'default']}
				>
					{props.children}
				</div>
			)}
		</div>
	);
};

export default Accordion;
