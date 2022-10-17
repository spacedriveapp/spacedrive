import { Select, SelectOption } from '@sd/ui';
import { useState } from 'react';

import Slider from '../primitive/Slider';

const Heading: React.FC<{ children: React.ReactNode }> = ({ children }) => (
	<div className="text-xs font-semibold text-gray-300">{children}</div>
);

const SubHeading: React.FC<{ children: React.ReactNode }> = ({ children }) => (
	<div className="mb-1 text-xs font-medium text-gray-300">{children}</div>
);

export const ExplorerOptionsPanel: React.FC = () => {
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
						<SelectOption value="name">Name</SelectOption>
						<SelectOption value="kind">Kind</SelectOption>
						<SelectOption value="favorite">Favorite</SelectOption>
						<SelectOption value="date_created">Date Created</SelectOption>
						<SelectOption value="date_modified">Date Modified</SelectOption>
						<SelectOption value="date_last_opened">Date Last Opened</SelectOption>
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
};
