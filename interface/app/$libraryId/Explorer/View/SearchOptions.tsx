import { Search } from 'react-router';
import { ObjectKind, useLibraryQuery } from '@sd/client';
import { Button, Input, Select, SelectOption, tw } from '@sd/ui';
import { getSearchStore, useSearchStore } from '~/hooks';

import TopBarButton from '../../TopBar/TopBarButton';

interface SearchOptionsProps {}

const Label = tw.span`text-ink-dull mr-2 text-xs`;
const OptionContainer = tw.div`flex flex-row items-center`;

const SearchOptions = (props: SearchOptionsProps) => {
	const { isSearching } = useSearchStore();
	const tags = useLibraryQuery(['tags.list']);
	return (
		<div className="sticky top-0 z-10 flex h-[45px] w-full flex-row items-center gap-4 border-b border-app-line/50 bg-app-darkBox px-4">
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
				<Select size="sm" onChange={(e) => {}} value={'directory'}>
					<SelectOption value="directory">This Directory</SelectOption>
					<SelectOption value="location">This Location</SelectOption>
					<SelectOption value="device">This Device</SelectOption>
					<SelectOption value="library">Entire Library</SelectOption>
				</Select>
			</OptionContainer>

			<OptionContainer>
				<Label>Kind:</Label>
				<Select onChange={(e) => {}} value={'all'}>
					<SelectOption value="all">All</SelectOption>
					{Object.values(ObjectKind).map(
						(kind) =>
							typeof kind !== 'number' && (
								<SelectOption key={kind} value={kind}>
									{kind}
								</SelectOption>
							)
					)}
				</Select>
			</OptionContainer>
			{tags.data && (
				<OptionContainer>
					<Label>Tagged:</Label>
					<Select onChange={(e) => {}} value={'any'}>
						<SelectOption value="any">Any</SelectOption>
						{tags.data.map(
							(tag) =>
								tag.name && (
									<SelectOption key={tag.id} value={tag.name}>
										{tag.name}
									</SelectOption>
								)
						)}
					</Select>
				</OptionContainer>
			)}
			{/* <OptionContainer>
				<Label>From:</Label>
				<Input size="xs" />
			</OptionContainer> */}
			<OptionContainer>
				<Label>Filter:</Label>
				<Select size="sm" onChange={(e) => {}} value={'kind'}>
					<SelectOption value="kind">Kind</SelectOption>
					<SelectOption value="extension">Extension</SelectOption>
					<SelectOption value="before_date">Before Date</SelectOption>
					<SelectOption value="after_date">After Date</SelectOption>
					<SelectOption value="date_range">Date Range</SelectOption>
					{/* <SelectOption value="library">Entire Library</SelectOption> */}
				</Select>
			</OptionContainer>
			<OptionContainer>
				<Label>Properties:</Label>
				<Select size="sm" onChange={(e) => {}} value={'name'}>
					<SelectOption value="name">Name</SelectOption>
					<SelectOption value="extension">Extension</SelectOption>
					<SelectOption value="note">Note</SelectOption>
					<SelectOption value="file_content">File Content</SelectOption>
				</Select>
			</OptionContainer>
			<Button size="xs" variant="gray">
				Add Filter
			</Button>
			<div className="flex-grow" />
			<Button size="xs" variant="gray" onClick={() => (getSearchStore().isSearching = false)}>
				Close
			</Button>
		</div>
	);
};

export default SearchOptions;
