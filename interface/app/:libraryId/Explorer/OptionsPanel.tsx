import { useState } from 'react';
import { Select, SelectOption, Slider, tw } from '@sd/ui';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';

const Heading = tw.div`text-ink-dull text-xs font-semibold`;
const Subheading = tw.div`text-ink-dull mb-1 text-xs font-medium`;

const sortOptions = {
	name: 'Name',
	kind: 'Kind',
	favorite: 'Favorite',
	date_created: 'Date Created',
	date_modified: 'Date Modified',
	date_last_opened: 'Date Last Opened'
};

export default () => {
	const [sortBy, setSortBy] = useState('name');
	const [stackBy, setStackBy] = useState('kind');

	const explorerStore = useExplorerStore();

	return (
		<div className="p-4 ">
			{/* <Heading>Explorer Appearance</Heading> */}
			<Subheading>Item size</Subheading>
			<Slider
				onValueChange={(value) => {
					getExplorerStore().gridItemSize = value[0] || 100;
					console.log({ value: value, gridItemSize: explorerStore.gridItemSize });
				}}
				defaultValue={[explorerStore.gridItemSize]}
				max={200}
				step={10}
				min={60}
			/>
			<div className="my-2 mt-4 grid grid-cols-2 gap-2">
				<div className="flex flex-col">
					<Subheading>Sort by</Subheading>
					<Select value={sortBy} size="sm" onChange={setSortBy}>
						{Object.entries(sortOptions).map(([value, text]) => (
							<SelectOption key={value} value={value}>
								{text}
							</SelectOption>
						))}
					</Select>
				</div>
				<div className="flex flex-col">
					<Subheading>Stack by</Subheading>
					<Select value={stackBy} size="sm" onChange={setStackBy}>
						<SelectOption value="kind">Kind</SelectOption>
						<SelectOption value="location">Location</SelectOption>
						<SelectOption value="node">Node</SelectOption>
					</Select>
				</div>
			</div>
		</div>
	);
};
