import * as icons from '@sd/assets/icons';
import byteSize from 'byte-size';
import clsx from 'clsx';
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import { Statistics, useLibraryContext, useLibraryQuery } from '@sd/client';
import { Card, ScreenHeading, Select, SelectOption } from '@sd/ui';
import { useExplorerTopBarOptions } from '~/hooks';
import useCounter from '~/hooks/useCounter';
import { usePlatform } from '~/util/Platform';
import Explorer from './Explorer';
import TopBarChildren from './TopBar/TopBarChildren';

interface StatItemProps {
	title: string;
	bytes: bigint;
	isLoading: boolean;
}

const StatItemNames: Partial<Record<keyof Statistics, string>> = {
	total_bytes_capacity: 'Total capacity',
	preview_media_bytes: 'Preview media',
	library_db_size: 'Index size',
	total_bytes_free: 'Free space'
};

const EMPTY_STATISTICS = {
	id: 0,
	date_captured: '',
	total_bytes_capacity: '0',
	preview_media_bytes: '0',
	library_db_size: '0',
	total_object_count: 0,
	total_bytes_free: '0',
	total_bytes_used: '0',
	total_unique_bytes: '0'
};

const displayableStatItems = Object.keys(StatItemNames) as unknown as keyof typeof StatItemNames;

let overviewMounted = false;

const StatItem = (props: StatItemProps) => {
	const { title, bytes = BigInt('0'), isLoading } = props;

	const size = byteSize(Number(bytes)); // TODO: This BigInt to Number conversion will truncate the number if the number is too large. `byteSize` doesn't support BigInt so we are gonna need to come up with a longer term solution at some point.
	const count = useCounter({
		name: title,
		end: +size.value,
		duration: overviewMounted ? 0 : 1,
		saveState: false
	});

	return (
		<div
			className={clsx(
				'flex w-32 shrink-0 cursor-default flex-col rounded-md px-4 py-3 duration-75',
				!bytes && 'hidden'
			)}
		>
			<span className="text-sm text-gray-400">{title}</span>
			<span className="text-2xl">
				{isLoading && (
					<div>
						<Skeleton
							enableAnimation={true}
							baseColor={'#21212e'}
							highlightColor={'#13131a'}
						/>
					</div>
				)}
				<div
					className={clsx({
						hidden: isLoading
					})}
				>
					<span className="font-black tabular-nums">{count}</span>
					<span className="ml-1 text-[16px] text-gray-400">{size.unit}</span>
				</div>
			</span>
		</div>
	);
};

export const Component = () => {
	const platform = usePlatform();
	const { library } = useLibraryContext();
	const stats = useLibraryQuery(['library.getStatistics'], {
		initialData: { ...EMPTY_STATISTICS }
	});
	const { explorerViewOptions } = useExplorerTopBarOptions();
	const recentFiles = useLibraryQuery(['files.getRecent', 50]);

	overviewMounted = true;

	const toolsViewOptions = explorerViewOptions.filter(
		(o) =>
			o.toolTipLabel === 'Grid view' ||
			o.toolTipLabel === 'List view' ||
			o.toolTipLabel === 'Media view'
	);

	return (
		<div className="flex">
			<TopBarChildren toolOptions={[toolsViewOptions]} />
			<div className="flex h-screen w-full flex-col">
				{/* STAT HEADER */}
				<div className="flex w-full">
					{/* STAT CONTAINER */}
					<div className="-mb-1 flex h-20 overflow-hidden">
						{Object.entries(stats?.data || []).map(([key, value]) => {
							if (!displayableStatItems.includes(key)) return null;
							return (
								<StatItem
									key={`${library.uuid} ${key}`}
									title={StatItemNames[key as keyof Statistics]!}
									bytes={BigInt(value)}
									isLoading={platform.demoMode ? false : stats.isLoading}
								/>
							);
						})}
					</div>
				</div>
			<div className="mt-4 flex flex-wrap space-x-[1px]">
				<CategoryButton selected icon={icons.Collection} category="Recents" items={1} />
				{/* <CategoryButton icon={icons.Node} category="Nodes" items={1} />
				<CategoryButton icon={icons.Folder} category="Locations" items={2} /> */}
				<CategoryButton icon={icons.Video} category="Movies" items={345} />
				<CategoryButton icon={icons.Audio} category="Music" items={54} />
				<CategoryButton icon={icons.Image} category="Pictures" items={908} />
				<CategoryButton icon={icons.EncryptedLock} category="Encrypted" items={3} />
				<CategoryButton icon={icons.Package} category="Downloads" items={89} />
			</div>


				{/* Recents */}
				{(recentFiles.data?.length || 0) > 0 && (
					<>
						{/* <ScreenHeading className="mt-3">Recents</ScreenHeading> */}
						<Explorer viewClassName="!pl-0 !pt-2" items={recentFiles.data} />
					</>
				)}
			</div>
			</div>

	);
};

interface CategoryButtonProps {
	category: string;
	items: number;
	icon: string;
	selected?: boolean;
}

function CategoryButton({ category, icon, items, selected }: CategoryButtonProps) {
	return (
		<div className={clsx("flex shrink-0 items-center rounded-md px-1.5 py-1 text-sm", selected && "bg-app-selected/20")}>
			<img src={icon} className="mr-3 h-12 w-12" />
			<div className="pr-5">
				<h2 className="text-sm font-medium">{category}</h2>
				{items !== undefined && (
					<p className="text-xs text-ink-faint">
						{items} Item{items > 1 && 's'}
					</p>
				)}
			</div>
		</div>
	);
}
