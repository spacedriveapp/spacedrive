import { Devices } from './Devices';
import { Locations } from './Locations';
import { SavedSearches } from './SavedSearches';
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
