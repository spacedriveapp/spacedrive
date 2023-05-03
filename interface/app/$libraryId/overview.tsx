import * as icons from '@sd/assets/icons';
import byteSize from 'byte-size';
import clsx from 'clsx';
import Skeleton from 'react-loading-skeleton';
import 'react-loading-skeleton/dist/skeleton.css';
import { Statistics, useLibraryContext, useLibraryQuery } from '@sd/client';
import { Card, ScreenHeading, Select, SelectOption } from '@sd/ui';
import useCounter from '~/hooks/useCounter';
import { usePlatform } from '~/util/Platform';

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

	overviewMounted = true;

	return (
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
			<div className="mt-4 flex flex-wrap">
				<CategoryButton icon={icons.Node} category="Nodes" items={1} />
				<CategoryButton icon={icons.Folder} category="Locations" items={2} />
				<CategoryButton icon={icons.Video} category="Movies" items={345} />
				<CategoryButton icon={icons.Audio} category="Music" items={54} />
				<CategoryButton icon={icons.Image} category="Pictures" items={908} />
				<CategoryButton icon={icons.EncryptedLock} category="Encrypted" items={3} />
				<CategoryButton icon={icons.Package} category="Downloads" items={89} />
				{/* <CategoryButton icon={FileText} category="Documents" />
				<CategoryButton icon={Camera} category="Movies" />
				<CategoryButton icon={FrameCorners} category="Screenshots" />
				<CategoryButton icon={AppWindow} category="Applications" />
				<CategoryButton icon={Wrench} category="Projects" />
				<CategoryButton icon={CloudArrowDown} category="Downloads" />
				<CategoryButton icon={MusicNote} category="Music" />
				<CategoryButton icon={Image} category="Albums" />
				<CategoryButton icon={Heart} category="Favorites" /> */}
			</div>
			<div className="flex h-4 w-full shrink-0" />
		</div>
	);
};

interface CategoryButtonProps {
	category: string;
	items: number;
	icon: string;
}

function CategoryButton({ category, icon, items }: CategoryButtonProps) {
	return (
		<div className="flex shrink-0 items-center hover:bg-app-box/50 rounded-md px-1.5 py-1 text-sm">
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
