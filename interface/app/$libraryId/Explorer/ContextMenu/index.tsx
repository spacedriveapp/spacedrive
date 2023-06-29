import { ExplorerItem } from '@sd/client';
import FilePathCM from './FilePath';
import LocationCM from './Location';
import ObjectCM from './Object';

export * as SharedItems from './SharedItems';
export * as FilePathItems from './FilePath/Items';
export * as ObjectItems from './Object/Items';

export default ({ item }: { item?: ExplorerItem }) => {
	if (!item) return null;

	switch (item.type) {
		case 'Path':
			return <FilePathCM data={item} />;
		case 'Object':
			return <ObjectCM data={item} />;
		case 'Location':
			return <LocationCM data={item} />;
	}
};
