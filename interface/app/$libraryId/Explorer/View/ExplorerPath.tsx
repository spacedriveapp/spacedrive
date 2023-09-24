import { CaretRight } from '@phosphor-icons/react';
import { memo, useCallback, useEffect, useState } from 'react';
import { useLocation } from 'react-router';
import { ExplorerItem } from '@sd/client';
import { getIcon } from '~/../packages/assets/util';
import { SearchParamsSchema } from '~/app/route-schemas';
import { useIsDark, useZodSearchParams } from '~/hooks';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerSearchParams } from '../util';

export const ExplorerPath = memo(() => {
	const location = useLocation();
	const isDark = useIsDark();
	const isEphemeralLocation = location.pathname.split('/').includes('ephemeral');

	const [data, setData] = useState<{ kind: string; name: string }[] | null>(null);
	const [selectedItem, setSelectedItem] = useState<ExplorerItem | undefined>(undefined);

	const explorerContext = useExplorerContext();
	const [{ path }] = useExplorerSearchParams();
	const [_, setSearchParams] = useZodSearchParams(SearchParamsSchema);

	const indexedPath =
		explorerContext.parent?.type === 'Location' && explorerContext.parent.location.path;

	//There are cases where the path ends with a '/' and cases where it doesn't
	const pathInfo = indexedPath
		? indexedPath + (path ? path.slice(0, -1) : '')
		: path?.endsWith('/')
		? path?.slice(0, -1)
		: path;

	const pathBuilder = (pathsToSplit: string, clickedPath: string): string => {
		const splitPaths = pathsToSplit?.split('/');
		const indexOfClickedPath = splitPaths?.indexOf(clickedPath);
		const newPath = splitPaths?.slice(0, (indexOfClickedPath as number) + 1).join('/') + '/';
		return newPath;
	};

	const pathRedirectHandler = (pathName: string): void => {
		const isPathNameEqualLocationName =
			pathName ===
			(explorerContext.parent?.type === 'Location' && explorerContext.parent?.location.name);

		if (isEphemeralLocation) {
			const getPaths = data?.map((p) => p.name).join('/');
			const newPath = `/${pathBuilder(getPaths as string, pathName)}`;
			return setSearchParams((p) => ({ ...p, path: newPath }), {
				replace: true
			});
		}
		const newPath = pathBuilder(path as string, pathName);
		setSearchParams((p) => ({ ...p, path: isPathNameEqualLocationName ? '' : newPath }), {
			replace: true
		});
	};

	const formatPathData = useCallback(() => {
		if (!pathInfo) return;

		const pathNameLocationName = (explorerContext.parent?.type === 'Location' &&
			explorerContext.parent?.location.name) as string;
		const splitPaths = pathInfo.split('/');
		const startIndex = isEphemeralLocation ? 1 : splitPaths.indexOf(pathNameLocationName);

		const updatedPathData = splitPaths.slice(startIndex);
		const updatedData = updatedPathData.map((path) => ({
			kind: 'Folder',
			extension: '',
			name: path
		}));

		setData(updatedData);
	}, [pathInfo, isEphemeralLocation]);

	useEffect(() => {
		formatPathData();
		const [first] = explorerContext.selectedItems;
		if (explorerContext.selectedItems.size === 1) {
			setSelectedItem(first);
		} else setSelectedItem(undefined);
	}, [pathInfo, explorerContext.selectedItems, formatPathData]);

	return (
		<div
			className="fixed bottom-0 flex h-8 w-full items-center gap-1  border-t
		border-t-app-line bg-app/90 px-3.5 text-[11px] text-ink-faint backdrop-blur-lg"
		>
			{data?.map((p, index) => {
				return (
					<div
						onClick={() => pathRedirectHandler(p.name)}
						key={p.name}
						className="flex items-center gap-1 transition-all duration-300 hover:brightness-125"
					>
						<img src={getIcon('Folder', isDark)} alt="folder" className="h-3 w-3" />
						<p className="truncate">{p.name}</p>
						{index !== (data?.length as number) - 1 && (
							<CaretRight weight="bold" size={10} />
						)}
					</div>
				);
			})}
			{selectedItem && (
				<div className="flex items-center gap-1 transition-all duration-300 hover:brightness-125">
					{data && data.length > 0 && <CaretRight weight="bold" size={10} />}
					<FileThumb size={12} data={selectedItem} />
					{'name' in selectedItem.item && <p>{selectedItem.item.name}</p>}
				</div>
			)}
		</div>
	);
});
