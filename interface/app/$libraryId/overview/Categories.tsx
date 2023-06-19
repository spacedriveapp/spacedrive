import { getIcon } from '@sd/assets/util';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { ArrowLeft, ArrowRight } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import { Category, useLibraryQuery } from '@sd/client';
import { useIsDark } from '~/hooks';
import CategoryButton from './CategoryButton';
import { IconForCategory } from './data';

const CategoryList = [
	'Recents',
	'Favorites',
	'Photos',
	'Videos',
	'Movies',
	'Music',
	'Documents',
	'Downloads',
	'Encrypted',
	'Projects',
	'Applications',
	'Archives',
	'Databases',
	'Games',
	'Books',
	'Contacts',
	'Trash'
] as Category[];

export const Categories = (props: { selected: Category; onSelectedChanged(c: Category): void }) => {
	const categories = useLibraryQuery(['categories.list']);
	const isDark = useIsDark();
	const [scroll, setScroll] = useState(0);
	const ref = useRef<HTMLDivElement>(null);
	const [lastCategoryVisible, setLastCategoryVisible] = useState(false);

	useEffect(() => {
		const element = ref.current;
		if (!element) return;
		const handler = () => {
			setScroll(element.scrollLeft);
		};
		element.addEventListener('scroll', handler);
		return () => {
			element.removeEventListener('scroll', handler);
		};
	}, []);

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

	return (
		<div className="sticky top-0 z-10 mt-2 flex bg-app/90 backdrop-blur">
			<div
				onClick={() => handleArrowOnClick('right')}
				className={clsx(
					scroll > 0
						? 'cursor-pointer bg-app/50 opacity-100 hover:opacity-95'
						: 'pointer-events-none',
					'sticky left-[15px] z-40 mt-4 flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-app-line bg-app p-2 opacity-0 backdrop-blur-md transition-all duration-200'
				)}
			>
				<ArrowLeft weight="bold" className="h-4 w-4 text-ink" />
			</div>
			<div
				ref={ref}
				className="no-scrollbar flex space-x-[1px] overflow-x-scroll py-1.5 pl-0 pr-[60px]"
				style={{
					maskImage: `linear-gradient(90deg, transparent 0.1%, rgba(0, 0, 0, 1) ${scroll > 0 ? '10%' : '0%'
						}, rgba(0, 0, 0, 1) ${lastCategoryVisible ? '95%' : '85%'}, transparent 99%)`
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
								className="min-w-fit"
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
			<div
				onClick={() => handleArrowOnClick('left')}
				className={clsx(
					lastCategoryVisible
						? 'pointer-events-none opacity-0 hover:opacity-0'
						: 'hover:opacity-95',
					'sticky right-[15px] z-40 mt-4 flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-full border border-app-line bg-app/50 p-2 backdrop-blur-md transition-all duration-200'
				)}
			>
				<ArrowRight weight="bold" className="h-4 w-4 text-ink" />
			</div>
		</div>
	);
};
