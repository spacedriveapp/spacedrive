import { Devices } from './Devices';
import { Locations } from './Locations';
import { SavedSearches } from './Saved';
import { Tags } from './Tags';

export const LibrarySection = () => {
	return (
		<>
			<SavedSearches />
			<Devices />
			<Locations />
			<Tags />
		</>
	);
};
