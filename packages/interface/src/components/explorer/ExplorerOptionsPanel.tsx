import { Select, SelectOption } from '@sd/ui';
import { PropsWithChildren, useState } from 'react';

import Slider from '../primitive/Slider';

function Heading({ children }: PropsWithChildren) {
	return <div className="text-xs font-semibold text-ink-dull">{children}</div>;
}

function SubHeading({ children }: PropsWithChildren) {
	return <div className="mb-1 text-xs font-medium text-ink-dull">{children}</div>;
}

const sortOptions = {
	name: 'Name',
	kind: 'Kind',
	favorite: 'Favorite',
	date_created: 'Date Created',
	date_modified: 'Date Modified',
	date_last_opened: 'Date Last Opened'
};

export function ExplorerOptionsPanel() {
	const [sortBy, setSortBy] = useState('name');
	const [stackBy, setStackBy] = useState('kind');
	const [size, setSize] = useState([50]);

	return (
		<div className="p-4 ">
			{/* <Heading>Explorer Appearance</Heading> */}
			<SubHeading>Item size</SubHeading>
			<Slider defaultValue={size} step={10} />
			<div className="grid grid-cols-2 gap-2 my-2 mt-4">
				<div className="flex flex-col">
					<SubHeading>Sort by</SubHeading>
					<Select value={sortBy} size="sm" onChange={setSortBy}>
						{Object.entries(sortOptions).map(([value, text]) => (
							<SelectOption key={value} value={value}>
								{text}
							</SelectOption>
						))}
					</Select>
				</div>
				<div className="flex flex-col">
					<SubHeading>Stack by</SubHeading>
					<Select value={stackBy} size="sm" onChange={setStackBy}>
						<SelectOption value="kind">Kind</SelectOption>
						<SelectOption value="location">Location</SelectOption>
						<SelectOption value="node">Node</SelectOption>
					</Select>
				</div>
			</div>
		</div>
	);
}
