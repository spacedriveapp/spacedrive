import { ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { ReactNode, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useDraggable } from 'react-use-draggable-scroll';
import { tw } from '@sd/ui';

const ArrowButton = tw.div`absolute top-1/2 z-40 flex h-8 w-8 shrink-0 -translate-y-1/2 items-center p-2 cursor-pointer justify-center rounded-full border border-app-line bg-app/50 hover:opacity-95 backdrop-blur-md transition-all duration-200`;

const HorizontalScroll = ({ children, className }: { children: ReactNode; className?: string }) => {
	const ref = useRef<HTMLDivElement>(null);
	const { events } = useDraggable(ref as React.MutableRefObject<HTMLDivElement>);
	const [lastItemVisible, setLastItemVisible] = useState(false);
	const [scroll, setScroll] = useState(0);
	// If the content is overflowing, we need to show the arrows
	const [isContentOverflow, setIsContentOverflow] = useState(false);

	const updateScrollState = () => {
		const element = ref.current;
		if (element) {
			setScroll(element.scrollLeft);
			setLastItemVisible(element.scrollWidth - element.clientWidth === element.scrollLeft);
			setIsContentOverflow(element.scrollWidth > element.clientWidth);
		}
	};

	useEffect(() => {
		const element = ref.current;
		if (element) {
			element.addEventListener('scroll', updateScrollState);
			window.addEventListener('resize', updateScrollState);
		}
		return () => {
			if (element) {
				element.removeEventListener('scroll', updateScrollState);
				window.removeEventListener('resize', updateScrollState);
			}
		};
	}, [ref]);

	useLayoutEffect(() => {
		updateScrollState();
	}, [ref.current?.clientWidth]);

	const handleArrowOnClick = (direction: 'right' | 'left') => {
		const element = ref.current;
		if (!element) return;

		const scrollAmount = element.clientWidth;

		element.scrollTo({
			left:
				direction === 'left'
					? element.scrollLeft + scrollAmount
					: element.scrollLeft - scrollAmount,
			behavior: 'smooth'
		});
	};

	const maskImage = `linear-gradient(90deg, transparent 0.1%, rgba(0, 0, 0, 1) ${
		scroll > 0 ? '10%' : '0%'
	}, rgba(0, 0, 0, 1) ${lastItemVisible ? '95%' : '85%'}, transparent 99%)`;

	return (
		<div className={clsx(className, 'relative mb-4')}>
			<ArrowButton
				onClick={() => handleArrowOnClick('right')}
				className={clsx('left-0 -ml-1', scroll === 0 && 'pointer-events-none opacity-0')}
			>
				<ArrowLeft weight="bold" className="size-4 text-ink" />
			</ArrowButton>
			<div
				ref={ref}
				{...events}
				className={clsx(
					'no-scrollbar flex gap-2 space-x-px overflow-x-scroll pl-1 pr-[30px]',
					isContentOverflow ? 'cursor-grab' : 'cursor-default'
				)}
				style={{
					WebkitMaskImage: maskImage,
					maskImage
				}}
			>
				{children}
			</div>

			{isContentOverflow && (
				<ArrowButton
					onClick={() => handleArrowOnClick('left')}
					className={clsx(
						'right-0 -mr-1',
						lastItemVisible && 'pointer-events-none opacity-0'
					)}
				>
					<ArrowRight weight="bold" className="size-4 text-ink" />
				</ArrowButton>
			)}
		</div>
	);
};

export default HorizontalScroll;
