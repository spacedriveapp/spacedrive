import clsx from 'clsx';
import { motion } from 'framer-motion';
import { RefObject, useEffect, useRef, useState } from 'react';
import { formatNumber, useLibraryQuery } from '@sd/client';
import { Icon } from '~/components';

import { useLayoutContext } from '../Layout/Context';
import HorizontalScroll from './HorizontalScroll';

export default () => {
	const ref = useRef<HTMLDivElement>(null);

	const { mouseState } = useMouseHandlers({ ref });

	const kinds = useLibraryQuery(['library.kindStatistics']);

	return (
		<>
			{/* This is awful, will replace icons accordingly and memo etc */}
			{kinds.data?.statistics
				?.sort((a, b) => b.count - a.count)
				.filter((i) => i.kind !== 0)
				.map(({ kind, name, count }) => {
					let icon = name;
					switch (name) {
						case 'Code':
							icon = 'Terminal';
							break;
						case 'Unknown':
							icon = 'Undefined';
							break;
					}
					return (
						<motion.div
							viewport={{
								root: ref,
								// WARNING: Edge breaks if the values are not postfixed with px or %
								margin: '0% -120px 0% 0%'
							}}
							className={clsx(
								'min-w-fit',
								mouseState !== 'dragging' && '!cursor-default'
							)}
							key={kind}
						>
							<KindItem name={name} icon={icon} items={count} onClick={() => {}} />
						</motion.div>
					);
				})}
		</>
	);
};

interface KindItemProps {
	name: string;
	items: number;
	icon: string;
	selected?: boolean;
	onClick?: () => void;
	disabled?: boolean;
}

const KindItem = ({ name, icon, items, selected, onClick, disabled }: KindItemProps) => {
	return (
		<div
			onClick={onClick}
			className={clsx(
				'flex shrink-0 items-center rounded-lg py-1 text-sm outline-none focus:bg-app-selectedItem/50',
				selected && 'bg-app-selectedItem',
				disabled && 'cursor-not-allowed opacity-30'
			)}
		>
			<Icon name={icon as any} className="mr-3 h-12 w-12" />
			<div className="pr-5">
				<h2 className="text-sm font-medium">{name}</h2>
				{items !== undefined && (
					<p className="text-xs text-ink-faint">
						{formatNumber(items)} Item{(items > 1 || items === 0) && 's'}
					</p>
				)}
			</div>
		</div>
	);
};

const useMouseHandlers = ({ ref }: { ref: RefObject<HTMLDivElement> }) => {
	const layout = useLayoutContext();

	const [scroll, setScroll] = useState(0);

	type MouseState = 'idle' | 'mousedown' | 'dragging';
	const [mouseState, setMouseState] = useState<MouseState>('idle');

	useEffect(() => {
		const element = ref.current;
		if (!element) return;

		const onScroll = () => {
			setScroll(element.scrollLeft);

			setMouseState((s) => {
				if (s !== 'mousedown') return s;

				if (layout.ref.current) layout.ref.current.style.cursor = 'grabbing';

				return 'dragging';
			});
		};
		const onWheel = (event: WheelEvent) => {
			event.preventDefault();
			const { deltaX, deltaY } = event;
			const scrollAmount = Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;
			element.scrollTo({ left: element.scrollLeft + scrollAmount });
		};
		const onMouseDown = () => setMouseState('mousedown');

		const onMouseUp = () => {
			setMouseState('idle');
			if (layout.ref.current) {
				layout.ref.current.style.cursor = '';
			}
		};

		element.addEventListener('scroll', onScroll);
		element.addEventListener('wheel', onWheel);
		element.addEventListener('mousedown', onMouseDown);

		window.addEventListener('mouseup', onMouseUp);

		return () => {
			element.removeEventListener('scroll', onScroll);
			element.removeEventListener('wheel', onWheel);
			element.removeEventListener('mousedown', onMouseDown);

			window.removeEventListener('mouseup', onMouseUp);
		};
	}, [ref, layout.ref]);

	return { scroll, mouseState };
};
