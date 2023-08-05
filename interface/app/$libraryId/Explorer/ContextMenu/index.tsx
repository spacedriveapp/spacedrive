import { ReactNode } from 'react';
import { ExplorerItem, FilePath, Location, Object } from '@sd/client';
import FilePathCM from './FilePath';
import LocationCM from './Location';
import ObjectCM from './Object';

export * as FilePathItems from './FilePath/Items';
export * as ObjectItems from './Object/Items';
export * as SharedItems from './SharedItems';

export type ExtraFn = (a: {
	object?: Object;
	filePath?: FilePath;
	location?: Location;
}) => ReactNode;

export default ({ item, extra }: { item: ExplorerItem; extra?: ExtraFn }) => {
	switch (item.type) {
		case 'Path':
			return <FilePathCM data={item} extra={extra} />;
		case 'Object':
			return <ObjectCM data={item} extra={extra} />;
		case 'Location':
			return <LocationCM data={item} extra={extra} />;
	}
};
