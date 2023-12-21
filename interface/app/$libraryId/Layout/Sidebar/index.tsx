import { LibraryContextProvider, useClientContext } from '@sd/client';

import SidebarLayout from './Layout';
import Debug from './sections/Debug';
// sections
import Devices from './sections/Devices';
import Library from './sections/Library';
import Local from './sections/Local';
import Locations from './sections/Locations';
import SavedSearches from './sections/SavedSearches';
import Tags from './sections/Tags';

export default function Sidebar() {
	const { library } = useClientContext();
	return (
		<SidebarLayout>
			{library && (
				<LibraryContextProvider library={library}>
					<Library />
				</LibraryContextProvider>
			)}
			<Local />
			<Debug />
			{library && (
				<LibraryContextProvider library={library}>
					<SavedSearches />
					<Devices />
					<Locations />
					<Tags />
					{/* <Tools /> */}
				</LibraryContextProvider>
			)}
		</SidebarLayout>
	);
}
