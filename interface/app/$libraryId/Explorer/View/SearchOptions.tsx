import { FunnelSimple, GearSix, Option, Sliders } from '@phosphor-icons/react';
import { useState } from 'react';
import { Search } from 'react-router';
import { ObjectKind, useLibraryQuery } from '@sd/client';
import { Button, DropdownMenu, Input, Popover, Select, SelectOption, tw } from '@sd/ui';
import { getSearchStore, useKeybind, useSearchStore } from '~/hooks';

import MultiCheckbox from '../../../../components/MultiCheckbox';
import TopBarButton from '../../TopBar/TopBarButton';

interface SearchOptionsProps {}

const Label = tw.span`text-ink-dull mr-2 text-xs`;
const OptionContainer = tw.div`flex flex-row items-center`;

// This defines alternate layouts for the search options
// type CustomFilterType = 'none' | 'date' | 'contents';

type DateOption = 'before' | 'after' | 'exactly' | 'today' | 'within_last';

type CustomFilterOptions = 'none' | 'created_date' | 'modified_date' | 'indexed_date';

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

const SearchOptions = (props: SearchOptionsProps) => {
	const [customFilterOption, setCustomFilterOption] = useState<CustomFilterOptions>('none');

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
			className="sticky top-0 z-10 flex h-[45px] w-full flex-row items-center gap-4 border-b border-app-line/50 bg-app-darkBox px-4"
		>
			<OptionContainer className="flex flex-row items-center">
				<Label>Show:</Label>
				<Button size="xs" variant="accent" rounding="left">
					Paths
				</Button>
				<Button size="xs" variant="gray" rounding="right">
					Objects
				</Button>
			</OptionContainer>
			<OptionContainer>
				<Label>In:</Label>
				<Select
					size="sm"
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
				<Button className="flex flex-row gap-1" size="xs" variant="dotted">
					<FunnelSimple />
					Filter
				</Button>
			</OptionContainer>
			<div className="flex-grow" />
			{/* <OptionContainer>
				<Button className="flex flex-row gap-1" size="xs" variant="dotted">
					Save
				</Button>
			</OptionContainer> */}

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
