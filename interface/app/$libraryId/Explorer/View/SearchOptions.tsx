import { Search } from 'react-router';
import { ObjectKind, useLibraryQuery } from '@sd/client';
import { Button, Select, SelectOption, tw } from '@sd/ui';
import { useSearchStore } from '~/hooks';

import TopBarButton from '../../TopBar/TopBarButton';

interface SearchOptionsProps {}

const Label = tw.span`text-ink-dull mr-2 text-xs`;
const OptionContainer = tw.div`flex flex-row items-center`;

const SearchOptions = (props: SearchOptionsProps) => {
	const { isFocused } = useSearchStore();
	const tags = useLibraryQuery(['tags.list']);
	return (
		<div className="flex h-[45px] w-full flex-row items-center gap-4 border-b border-app-line/50 bg-app-box/50 px-4">
			<OptionContainer className="flex flex-row items-center">
				<Label>Show:</Label>
				<Button size="xs" variant="accent" rounding="left">
					Objects
				</Button>
				<Button size="xs" variant="gray" rounding="right">
					Paths
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
			{/* <Button variant="gray">Something</Button> */}
		</div>
	);
};

export default SearchOptions;
