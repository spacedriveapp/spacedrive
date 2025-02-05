import { LibraryContextProvider, useClientContext } from '@sd/client';

import Categories from './sections/Categories';
import Debug from './sections/Debug';
// sections
import Devices from './sections/Devices';
import Library from './sections/Library';
import Local from './sections/Local';
import Locations from './sections/Locations';
import Peers from './sections/Peers';
import SavedSearches from './sections/SavedSearches';
import Tags from './sections/Tags';
import Tools from './sections/Tools';
import SidebarLayout from './SidebarLayout';

export default function Sidebar() {
	const { library } = useClientContext();
	return (
		<SidebarLayout>
			{library && (
				<LibraryContextProvider library={library}>
					<Library />
				</LibraryContextProvider>
			)}
			<Debug />
			<Local />
			{library && (
				<LibraryContextProvider library={library}>
					<SavedSearches />
					{/* <Devices /> */}
					<Locations />
					{/* <Categories /> */}
					<Peers />
					<Tags />
				</LibraryContextProvider>
			)}
			<Tools />
		</SidebarLayout>
	);
}
