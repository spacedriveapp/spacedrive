import { CaretRight } from '@phosphor-icons/react';
import { getIcon } from '@sd/assets/util';
import clsx from 'clsx';
import { memo, useCallback, useEffect, useState } from 'react';
import { useLocation } from 'react-router';
import { ExplorerItem, getExplorerLayoutStore, useExplorerLayoutStore } from '@sd/client';
import { SearchParamsSchema } from '~/app/route-schemas';
import { useIsDark, useKeyBind, useKeyMatcher, useZodSearchParams } from '~/hooks';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerSearchParams } from '../util';

export const ExplorerPath = memo(() => {
	const location = useLocation();
	const isDark = useIsDark();
	const isEphemeralLocation = location.pathname.split('/').includes('ephemeral');

	const [data, setData] = useState<{ kind: string; name: string }[] | null>(null);
	const [selectedItem, setSelectedItem] = useState<ExplorerItem | undefined>(undefined);
	const metaCtrlKey = useKeyMatcher('Meta').key;
	const layoutStore = useExplorerLayoutStore();

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

	const pathRedirectHandler = (pathName: string, index: number): void => {
		if (isEphemeralLocation) {
			const getPaths = data?.map((p) => p.name).join('/');
			const newPath = `/${pathBuilder(getPaths as string, pathName)}`;
			return setSearchParams((p) => ({ ...p, path: newPath }), {
				replace: true
			});
		}
		const newPath = pathBuilder(path as string, pathName);
		setSearchParams((p) => ({ ...p, path: index === 0 ? '' : newPath }), {
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

	useKeyBind([metaCtrlKey, 'p'], (e) => {
		e.stopPropagation();
		getExplorerLayoutStore().showPathBar = !layoutStore.showPathBar;
	});

	if (!layoutStore.showPathBar) return null;

	return (
		<div
			className="fixed bottom-0 flex h-8 w-full items-center gap-1  border-t
		border-t-app-line bg-app/90 px-3.5 text-[11px] text-ink-faint backdrop-blur-lg"
		>
			{data?.map((p, index) => {
				return (
					<div
						onClick={() => pathRedirectHandler(p.name, index)}
						key={(p.name + index).toString()}
						className={clsx(
							'flex items-center gap-1 transition-all duration-300',
							index !== data.length - 1 && ' cursor-pointer hover:brightness-125'
						)}
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
				<div className="pointer-events-none flex items-center gap-1">
					{data && data.length > 0 && <CaretRight weight="bold" size={10} />}
					<FileThumb size={12} data={selectedItem} />
					{'name' in selectedItem.item && <p>{selectedItem.item.name}</p>}
				</div>
			)}
		</div>
	);
});
