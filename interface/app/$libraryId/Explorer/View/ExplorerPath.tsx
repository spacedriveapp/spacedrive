import { CaretRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { memo, useCallback, useEffect, useState } from 'react';
import { useMatch } from 'react-router';
import { ExplorerItem } from '@sd/client';
import { SearchParamsSchema } from '~/app/route-schemas';
import { Icon } from '~/components';
import { useOperatingSystem, useZodSearchParams } from '~/hooks';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerSearchParams } from '../util';

export const PATH_BAR_HEIGHT = 32;

export const ExplorerPath = memo(() => {
	const isEphemeralLocation = useMatch('/:libraryId/ephemeral/:ephemeralId');
	const os = useOperatingSystem();
	const realOs = useOperatingSystem(true);
	const pathSlashOS = os === 'browser' ? '/' : realOs === 'windows' ? '\\' : '/';

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
		: path?.endsWith(pathSlashOS)
		? path?.slice(0, -1)
		: path;

	const pathBuilder = (pathsToSplit: string, clickedPath: string): string => {
		const slashCheck = isEphemeralLocation ? pathSlashOS : '/'; //in ephemeral locations, the path is built with '\' instead of '/' for windows
		const splitPaths = pathsToSplit?.split(slashCheck);
		const indexOfClickedPath = splitPaths?.indexOf(clickedPath);
		const newPath =
			splitPaths?.slice(0, (indexOfClickedPath as number) + 1).join(slashCheck) + slashCheck;
		return newPath;
	};

	const pathRedirectHandler = (pathName: string, index: number): void => {
		if (isEphemeralLocation) {
			const currentPaths = data?.map((p) => p.name).join(pathSlashOS);
			const newPath = `${pathSlashOS}${pathBuilder(currentPaths as string, pathName)}`;
			setSearchParams((params) => ({ ...params, path: newPath }), { replace: true });
		} else {
			const newPath = pathBuilder(path as string, pathName);
			setSearchParams((params) => ({ ...params, path: index === 0 ? '' : newPath }), {
				replace: true
			});
		}
	};

	const pathNameLocationName =
		explorerContext.parent?.type === 'Location' && explorerContext.parent?.location.name;
	const formatPathData = useCallback(() => {
		if (!pathInfo) return;
		const splitPaths = pathInfo.replaceAll('/', pathSlashOS).split(pathSlashOS); //replace all '/' with '\' for windows
		const startIndex = isEphemeralLocation
			? 1
			: pathNameLocationName
			? splitPaths.indexOf(pathNameLocationName)
			: -1;
		const updatedPathData = splitPaths.slice(startIndex);
		const updatedData = updatedPathData.map((path) => ({
			kind: 'Folder',
			extension: '',
			name: path
		}));
		setData(updatedData);
	}, [pathInfo, pathSlashOS, isEphemeralLocation, pathNameLocationName]);

	useEffect(() => {
		formatPathData();
		const [first] = explorerContext.selectedItems;
		if (explorerContext.selectedItems.size === 1) {
			setSelectedItem(first);
		} else setSelectedItem(undefined);
	}, [pathInfo, explorerContext.selectedItems, formatPathData]);

	return (
		<div
			className="absolute inset-x-0 bottom-0 flex items-center gap-1 border-t border-t-app-line bg-app/90 px-3.5 text-[11px] text-ink-faint backdrop-blur-lg"
			style={{ height: PATH_BAR_HEIGHT }}
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
						<Icon name="Folder" size={16} alt="Folder" />
						<span className="max-w-xs truncate">{p.name}</span>
						{index !== (data?.length as number) - 1 && (
							<CaretRight weight="bold" size={10} />
						)}
					</div>
				);
			})}
			{selectedItem && (
				<div className="pointer-events-none flex items-center gap-1">
					{data && data.length > 0 && <CaretRight weight="bold" size={10} />}
					<FileThumb size={16} frame frameClassName="!border" data={selectedItem} />
					{'name' in selectedItem.item && (
						<span className="max-w-xs truncate">{selectedItem.item.name}</span>
					)}
				</div>
			)}
		</div>
	);
});
