import { type ReactNode } from 'react';
import {
	type ExplorerItem,
	type FilePath,
	type Location,
	type NonIndexedPathItem,
	type Object
} from '@sd/client';
import EphemeralPathCM from './EphemeralPath';
import FilePathCM from './FilePath';
import LocationCM from './Location';
import ObjectCM from './Object';

export * as SharedItems from './SharedItems';
export * as FilePathItems from './FilePath/Items';
export * as ObjectItems from './Object/Items';

export type ExtraFn = (a: {
	object?: Object;
	filePath?: FilePath;
	location?: Location | NonIndexedPathItem;
}) => ReactNode;

export default ({ item, extra }: { item: ExplorerItem; extra?: ExtraFn }) => {
	switch (item.type) {
		case 'Path':
			return <FilePathCM data={item} extra={extra} />;
		case 'Object':
			return <ObjectCM data={item} extra={extra} />;
		case 'Location':
			return <LocationCM data={item} extra={extra} />;
		case 'NonIndexedPath':
			return <EphemeralPathCM data={item} extra={extra} />;
	}
};
