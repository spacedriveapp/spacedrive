import {
	CircleDashed,
	Clock,
	Cube,
	Devices,
	FileDoc,
	FilePlus,
	Folder,
	FunnelSimple,
	Icon,
	Image,
	Key,
	SelectionSlash,
	User
} from '@phosphor-icons/react';
import { IconTypes } from '@sd/assets/util';
import { PropsWithChildren, useState } from 'react';
import { useLibraryQuery } from '@sd/client';
import { Button, CheckBox, DropdownMenu, Input, Select, SelectOption, tw } from '@sd/ui';
import { getSearchStore, useKeybind, useSearchStore } from '~/hooks';

import { RenderIcon } from './util';

const Label = tw.span`text-ink-dull mr-2 text-xs`;
const OptionContainer = tw.div`flex flex-row items-center`;

// This defines alternate layouts for the search options
// type CustomFilterType = 'none' | 'date' | 'contents';

type DateOption = 'before' | 'after' | 'exactly' | 'today' | 'within_last';

type CustomFilterOptions = 'none' | 'created_date' | 'modified_date' | 'indexed_date';

const SEPARATOR_STYLES = `!border-app-line`;

interface SearchOptionItemProps extends PropsWithChildren {
	checkbox?: boolean;
	icon?: Icon | IconTypes;
}

const SearchOptionItem = (props: SearchOptionItemProps) => {
	return (
		<DropdownMenu.Item variant="dull" className="group">
			{props.checkbox && <CheckBox className="mr-2 text-ink-dull" />}
			<RenderIcon icon={props.icon} />
			{props.children}
		</DropdownMenu.Item>
	);
};

const SearchOptionSubMenu = (props: SearchOptionItemProps & { name?: string }) => {
	return (
		<DropdownMenu.SubMenu label={props.name} variant="dull" className="group">
			<RenderIcon icon={props.icon} />
			{props.children}
		</DropdownMenu.SubMenu>
	);
};

const SearchOptions = () => {
	const [customFilterOption, setCustomFilterOption] = useState<CustomFilterOptions>('none');

	const [searchContext, setSearchContext] = useState<'paths' | 'objects'>('paths');

	const handleMouseEnter = () => {
		getSearchStore().interactingWithSearchOptions = true;
	};

	const handleMouseLeave = () => {
		getSearchStore().interactingWithSearchOptions = false;
	};

	useKeybind(['Escape'], () => {
		getSearchStore().isSearching = false;
	});

	const { isSearching, searchScope } = useSearchStore();
	const tags = useLibraryQuery(['tags.list']);
	return (
		<div
			onMouseEnter={handleMouseEnter}
			onMouseLeave={handleMouseLeave}
			className="bg-app-darkerBox/90 flex h-[45px] w-full flex-row items-center gap-4 border-b border-app-line/50 px-4 backdrop-blur"
		>
			<OptionContainer className="flex flex-row items-center">
				<Label>Show:</Label>
				<Button
					onClick={() => setSearchContext('paths')}
					size="xs"
					variant={searchContext === 'paths' ? 'accent' : 'gray'}
					rounding="left"
				>
					Paths
				</Button>
				<Button
					onClick={() => setSearchContext('objects')}
					size="xs"
					variant={searchContext === 'objects' ? 'accent' : 'gray'}
					rounding="right"
				>
					Objects
				</Button>
			</OptionContainer>
			<OptionContainer>
				<Label>In:</Label>
				<Select
					size="sm"
					className="w-[130px]"
					onChange={(scope) => (getSearchStore().searchScope = scope)}
					value={searchScope}
				>
					<SelectOption value="directory">This Directory</SelectOption>
					<SelectOption value="location">This Location</SelectOption>
					<SelectOption value="device">This Device</SelectOption>
					<SelectOption value="library">Entire Library</SelectOption>
				</Select>
			</OptionContainer>
			<div className="mx-1 h-[15px] w-[1px] bg-app-line" />

			<OptionContainer>
				<DropdownMenu.Root
					className="!rounded-md border !border-app-line !bg-app-box"
					trigger={
						<Button className="flex flex-row gap-1" size="xs" variant="dotted">
							<FunnelSimple />
							Add Filter
						</Button>
					}
				>
					<Input autoFocus variant="transparent" placeholder="Filter..." />
					<DropdownMenu.Separator className={SEPARATOR_STYLES} />
					<SearchOptionItem icon={Folder}>In Location</SearchOptionItem>
					<SearchOptionSubMenu name="In Location" icon={Folder}>
						<Input autoFocus variant="transparent" placeholder="Filter..." />
						<DropdownMenu.Separator className={SEPARATOR_STYLES} />
						<SearchOptionItem icon={Folder}>Location 1</SearchOptionItem>
					</SearchOptionSubMenu>
					<SearchOptionItem icon={FileDoc}>Kind</SearchOptionItem>
					<SearchOptionItem icon={Cube}>Size</SearchOptionItem>
					<SearchOptionItem icon={CircleDashed}>Tagged</SearchOptionItem>
					<SearchOptionItem icon={FilePlus}>In File Contents</SearchOptionItem>
					<SearchOptionItem icon={Image}>In Album</SearchOptionItem>
					<SearchOptionItem icon={Devices}>On Device</SearchOptionItem>
					<SearchOptionItem icon={Key}>Encrypted with Key</SearchOptionItem>
					<SearchOptionItem icon={User}>Shared by</SearchOptionItem>
					<SearchOptionItem icon={Clock}>Created At</SearchOptionItem>
					<SearchOptionItem icon={Clock}>Modified At</SearchOptionItem>
					<SearchOptionItem icon={Clock}>Last Opened At</SearchOptionItem>
					<SearchOptionItem icon={Clock}>Taken At</SearchOptionItem>
					<SearchOptionItem icon={SelectionSlash}>Hidden</SearchOptionItem>
				</DropdownMenu.Root>
			</OptionContainer>
			<div className="flex-grow" />
			{/* <OptionContainer>
				<Button className="flex flex-row gap-1" size="xs" variant="dotted">
					Save
				</Button>
			</OptionContainer>
								<DropdownMenu.SubMenu>
						<Input autoFocus variant="transparent" placeholder="Filter..." />
						<DropdownMenu.Separator className={SEPARATOR_STYLES} />
						<SearchOptionItem name="In Location" icon={Folder} />
						<SearchOptionItem name="In Location" icon={Folder} />
						<SearchOptionItem name="In Location" icon={Folder} />
					</DropdownMenu.SubMenu>

			*/}

			<kbd
				onClick={() => (getSearchStore().isSearching = false)}
				className="ml-2 rounded-lg border border-app-line bg-app-box px-2 py-1 text-[10.5px] tracking-widest shadow"
			>
				ESC
			</kbd>
		</div>
	);
};

export default SearchOptions;

// const DateSearchFilter = () => {
// 	const [dateOption, setDateOption] = useState<DateOption>('within_last');
// 	const [primaryDate, setPrimaryDate] = useState<string>('');
// 	const [withinLast, setWithinLast] = useState<number>(1);
// 	const [customFilter, setCustomFilter] = useState<CustomFilter>('');
// 	return (
// 		<>
// 			<OptionContainer className="gap-2">
// 				<Label className="!m-0">Is:</Label>
// 				<Select size="sm" onChange={(item) => setDateOption(item)} value={dateOption}>
// 					<SelectOption value="within_last">Within Last</SelectOption>
// 					<SelectOption value="before">Before</SelectOption>
// 					<SelectOption value="after">After</SelectOption>
// 					<SelectOption value="exactly">Exactly</SelectOption>
// 					<SelectOption value="today">Today</SelectOption>
// 					<SelectOption value="yesterday">Yesterday</SelectOption>
// 					<SelectOption value="this_week">This Week</SelectOption>
// 					<SelectOption value="this_month">This Month</SelectOption>
// 					<SelectOption value="this_year">This Year</SelectOption>
// 					<SelectOption value="last_year">Last Year</SelectOption>
// 				</Select>
// 				{['after', 'before', 'exactly'].includes(dateOption) && (
// 					<Input
// 						type="date"
// 						size="xs"
// 						onChange={(e) => setPrimaryDate(e.target.value)}
// 						value={primaryDate}
// 					/>
// 				)}
// 				{['within_last'].includes(dateOption) && (
// 					<>
// 						<Input
// 							size="xs"
// 							type="number"
// 							className="w-12"
// 							inputElementClassName="!pr-0.5"
// 							onChange={(e) => setWithinLast(Number(e.target.value))}
// 							value={withinLast}
// 						/>
// 						<Select size="sm" onChange={(item) => {}} value={'days'}>
// 							<SelectOption value="days">Days</SelectOption>
// 							<SelectOption value="weeks">Weeks</SelectOption>
// 							<SelectOption value="months">Months</SelectOption>
// 							<SelectOption value="years">Years</SelectOption>
// 						</Select>
// 					</>
// 				)}
// 			</OptionContainer>
// 		</>
// 	);
// };
