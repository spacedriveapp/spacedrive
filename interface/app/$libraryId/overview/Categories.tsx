import { getIcon } from '@sd/assets/util';
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

	return (
		<div className="no-scrollbar sticky top-0 z-10 mt-2 flex space-x-[1px] overflow-x-scroll bg-app/90 px-5 py-1.5 backdrop-blur">
			{categories.data &&
				CategoryList.map((category) => {
					const iconString = IconForCategory[category] || 'Document';

					return (
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
		</div>
	);
};
