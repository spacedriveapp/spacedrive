import { getIcon } from '@sd/assets/util';
import clsx from 'clsx';
import { useInView } from 'framer-motion';
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
	const lastCategoryRef = useRef<HTMLDivElement>(null);

	//if the last category is visible - we hide the right arrow
	const isInView = useInView(lastCategoryRef, {
		amount: 1,
		root: ref.current
	});

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
			left: direction === 'left' ? element.scrollLeft + 250 : element.scrollLeft - 250,
			behavior: 'smooth'
		});
	};

	return (
		<div
			ref={ref}
			className="no-scrollbar sticky top-0 z-10 ml-[-30px] mt-2 flex space-x-[1px] overflow-x-scroll bg-app/90 px-5 py-1.5 backdrop-blur"
		>
			<div
				onClick={() => handleArrowOnClick('right')}
				className={clsx(
					scroll > 0
						? 'cursor-pointer bg-opacity-50 opacity-100 hover:opacity-80'
						: 'pointer-events-none',
					'sticky left-[33px] z-40 mt-2 flex h-9 w-9 items-center justify-center rounded-full border border-app-line bg-app p-2 opacity-0 backdrop-blur-md transition-all duration-200'
				)}
			>
				<ArrowLeft weight="bold" className="h-4 w-4 text-white" />
			</div>
			{categories.data &&
				CategoryList.map((category, index) => {
					const iconString = IconForCategory[category] || 'Document';
					return index === CategoryList.length - 1 ? (
						<div className="min-w-fit" ref={lastCategoryRef}>
							<CategoryButton
								key={category}
								category={category}
								icon={getIcon(iconString, isDark)}
								items={categories.data[category]}
								selected={props.selected === category}
								onClick={() => props.onSelectedChanged(category)}
							/>
						</div>
					) : (
						<CategoryButton
							key={category}
							category={category}
							icon={getIcon(iconString, isDark)}
							items={categories.data[category]}
							selected={props.selected === category}
							onClick={() => props.onSelectedChanged(category)}
						/>
					);
				})}
			<div
				onClick={() => handleArrowOnClick('left')}
				className={clsx(
					isInView ? 'pointer-events-none opacity-0 hover:opacity-0' : 'hover:opacity-80',
					'sticky right-[2px] z-40 mt-2 flex h-9 w-9 cursor-pointer items-center justify-center rounded-full border border-app-line bg-app bg-opacity-50 p-2 backdrop-blur-md transition-all duration-200'
				)}
			>
				<ArrowRight weight="bold" className="h-4 w-4 text-white" />
			</div>
		</div>
	);
};
