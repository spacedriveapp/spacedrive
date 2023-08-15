import { getIcon } from '@sd/assets/util';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { ArrowLeft, ArrowRight } from 'phosphor-react';
import { RefObject, useEffect, useRef, useState } from 'react';
import Sticky from 'react-sticky-el';
import { useDraggable } from 'react-use-draggable-scroll';
import { Category, useLibraryQuery } from '@sd/client';
import { tw } from '@sd/ui';
import { useIsDark } from '~/hooks';
import { useLayoutContext } from '../Layout/Context';
import { usePageLayoutContext } from '../PageLayout/Context';
import CategoryButton from './CategoryButton';
import { IconForCategory } from './data';

export const CategoryList = [
	'Recents',
	'Favorites',
	'Albums',
	'Photos',
	'Videos',
	'Movies',
	'Music',
	'Documents',
	'Downloads',
	'Encrypted',
	'Projects',
	'Applications',
	// 'Archives',
	'Databases',
	'Games',
	'Books',
	// 'Contacts',
	'Trash'
] as Category[];

const ArrowButton = tw.div`absolute top-1/2 z-40 flex h-8 w-8 shrink-0 -translate-y-1/2 items-center p-2 cursor-pointer justify-center rounded-full border border-app-line bg-app/50 hover:opacity-95 backdrop-blur-md transition-all duration-200`;

export const Categories = (props: { selected: Category; onSelectedChanged(c: Category): void }) => {
	const isDark = useIsDark();

	const { ref: pageRef } = usePageLayoutContext();

	const ref = useRef<HTMLDivElement>(null);
	const { events } = useDraggable(ref as React.MutableRefObject<HTMLDivElement>);

	const { scroll, mouseState } = useMouseHandlers({ ref });

	const categories = useLibraryQuery(['categories.list']);

	const [lastCategoryVisible, setLastCategoryVisible] = useState(false);

	const handleArrowOnClick = (direction: 'right' | 'left') => {
		const element = ref.current;
		if (!element) return;

		element.scrollTo({
			left: direction === 'left' ? element.scrollLeft + 200 : element.scrollLeft - 200,
			behavior: 'smooth'
		});
	};

	const lastCategoryVisibleHandler = (index: number) => {
		index === CategoryList.length - 1 && setLastCategoryVisible((prev) => !prev);
	};

	const maskImage = `linear-gradient(90deg, transparent 0.1%, rgba(0, 0, 0, 1) ${
		scroll > 0 ? '10%' : '0%'
	}, rgba(0, 0, 0, 1) ${lastCategoryVisible ? '95%' : '85%'}, transparent 99%)`;

	return (
		<Sticky
			scrollElement={pageRef.current || undefined}
			stickyClassName="z-20 !top-[46px]"
			topOffset={-46}
		>
			<div className="relative flex bg-app/90 px-3 py-1.5 backdrop-blur">
				<ArrowButton
					onClick={() => handleArrowOnClick('right')}
					className={clsx('left-3', scroll === 0 && 'pointer-events-none opacity-0')}
				>
					<ArrowLeft weight="bold" className="h-4 w-4 text-ink" />
				</ArrowButton>

				<div
					ref={ref}
					{...events}
					className="no-scrollbar flex space-x-px overflow-x-scroll pr-[60px]"
					style={{
						WebkitMaskImage: maskImage, // Required for Chromium based browsers
						maskImage
					}}
				>
					{categories.data &&
						CategoryList.map((category, index) => {
							const iconString = IconForCategory[category] || 'Document';
							return (
								<motion.div
									onViewportEnter={() => lastCategoryVisibleHandler(index)}
									onViewportLeave={() => lastCategoryVisibleHandler(index)}
									viewport={{
										root: ref,
										// WARNING: Edge breaks if the values are not postfixed with px or %
										margin: '0% -120px 0% 0%'
									}}
									className={clsx(
										'min-w-fit',
										mouseState !== 'dragging' && '!cursor-default'
									)}
									key={category}
								>
									<CategoryButton
										category={category}
										icon={getIcon(iconString, isDark)}
										items={categories.data[category]}
										selected={props.selected === category}
										onClick={() => props.onSelectedChanged(category)}
									/>
								</motion.div>
							);
						})}
				</div>

				<ArrowButton
					onClick={() => handleArrowOnClick('left')}
					className={clsx(
						'right-3',
						lastCategoryVisible && 'pointer-events-none opacity-0'
					)}
				>
					<ArrowRight weight="bold" className="h-4 w-4 text-ink" />
				</ArrowButton>
			</div>
		</Sticky>
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
