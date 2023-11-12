import { CaretRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { ComponentProps, memo, useCallback, useEffect, useMemo, useRef } from 'react';
import { useMatch, useNavigate } from 'react-router';
import { ExplorerItem, FilePath, FilePathWithObject, useLibraryQuery } from '@sd/client';
import { LibraryIdParamsSchema, SearchParamsSchema } from '~/app/route-schemas';
import { Icon } from '~/components';
import { useOperatingSystem, useZodRouteParams, useZodSearchParams } from '~/hooks';

import { useExplorerContext } from '../Context';
import { FileThumb } from '../FilePath/Thumb';
import { useExplorerSearchParams } from '../util';

export const PATH_BAR_HEIGHT = 32;

export const ExplorerPath = memo(() => {
	const isEphemeralLocation = useMatch('/:libraryId/ephemeral/:ephemeralId');
	const os = useOperatingSystem();
	const realOs = useOperatingSystem(true);
	const navigate = useNavigate();
	const libraryId = useZodRouteParams(LibraryIdParamsSchema).libraryId;
	const pathSlashOS = os === 'browser' ? '/' : realOs === 'windows' ? '\\' : '/';
	const firstRenderCached = useRef<null | boolean>(null);

	const explorerContext = useExplorerContext();
	const fullPathOnClick = explorerContext.parent?.type === 'Tag';
	const [{ path }] = useExplorerSearchParams();
	const [_, setSearchParams] = useZodSearchParams(SearchParamsSchema);
	const selectedItem = useMemo(() => {
		if (explorerContext.selectedItems.size !== 1) return;
		return [...explorerContext.selectedItems][0];
	}, [explorerContext.selectedItems]);

	// On initial render, check if the location is nested
	// If it is, remove the first instance of the location name from the path
	// This is to prevent the path bar from showing the location name twice
	const isLocationNested = useCallback(() => {
		if (!explorerContext.parent || explorerContext.parent.type !== 'Location') return false;
		firstRenderCached.current = true;
		const { path: locationPath, name: locationName } = explorerContext.parent.location || {};

		if (!locationPath || !locationName) return false;
		const count = locationPath
			.split(pathSlashOS)
			.filter((part) => part === locationName).length;

		return count > 1;
	}, [explorerContext.parent, pathSlashOS]);

	// On the first render of a location, check if the location is nested
	useEffect(() => {
		if (explorerContext.parent?.type === 'Location') {
			isLocationNested();
		}
		return () => {
			firstRenderCached.current = null;
		};
	}, [explorerContext.parent, isLocationNested]);

	//this is being used with object page route - when clicking on an object

	const filePathData = () => {
		if (!selectedItem) return;
		let filePathData: FilePath | FilePathWithObject | null = null;
		const item = selectedItem as ExplorerItem;
		switch (item.type) {
			case 'Path': {
				filePathData = item.item;
				break;
			}
			case 'Object': {
				filePathData = item.item.file_paths[0] ?? null;
				break;
			}
			case 'SpacedropPeer': {
				// objectData = item.item as unknown as Object;
				// filePathData = item.item.file_paths[0] ?? null;
				break;
			}
		}
		return filePathData;
	};

	//this is being used with tag page route - when clicking on an object
	//we get the full path of the object and use it to build the path bar
	const queriedFullPath = useLibraryQuery(['files.getPath', filePathData()?.id ?? -1], {
		enabled: selectedItem != null && fullPathOnClick
	});

	const indexedPath = fullPathOnClick
		? queriedFullPath.data
		: explorerContext.parent?.type === 'Location' && explorerContext.parent.location.path;

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
		let newPath: string | undefined;
		if (fullPathOnClick) {
			if (!explorerContext.selectedItems) return;
			const objectData = Array.from(explorerContext.selectedItems)[0];
			if (!objectData) return;
			if ('file_paths' in objectData.item && objectData) {
				newPath = pathBuilder(pathInfo as string, pathName);
				navigate({
					pathname: `/${libraryId}/ephemeral/0`,
					search: `?path=${newPath}`
				});
			}
		} else if (isEphemeralLocation) {
			const currentPaths = data?.map((p) => p.name).join(pathSlashOS);
			newPath = `${pathSlashOS}${pathBuilder(currentPaths as string, pathName)}`;
			setSearchParams((params) => ({ ...params, path: newPath }));
		} else {
			newPath = pathBuilder(path as string, pathName);
			setSearchParams((params) => ({ ...params, path: index === 0 ? '' : newPath }));
		}
	};

	const pathNameLocationName =
		explorerContext.parent?.type === 'Location' && explorerContext.parent?.location.name;
	const data = useMemo(() => {
		if (!pathInfo) return;
		const splitPaths = pathInfo?.replaceAll('/', pathSlashOS).split(pathSlashOS); //replace all '/' with '\' for windows

		//if the path is a full path
		if (fullPathOnClick && queriedFullPath.data) {
			if (!selectedItem) return;
			const selectedItemFilePaths =
				'file_paths' in selectedItem.item && selectedItem.item.file_paths[0];
			if (!selectedItemFilePaths) return;
			const updatedData = splitPaths
				.map((path) => ({
					kind: 'Folder',
					extension: '',
					name: path
				}))
				//remove duplicate path names upon selection + from the result of the full path query
				.filter(
					(path) =>
						path.name !==
							`${selectedItemFilePaths.name}.${selectedItemFilePaths.extension}` &&
						path.name !== '' &&
						path.name !== selectedItemFilePaths.name
				);
			return updatedData;

			//handling ephemeral and location paths
		} else {
			let updatedPathData: string[] = [];
			const startIndex = isEphemeralLocation
				? 1
				: pathNameLocationName
				? splitPaths.indexOf(pathNameLocationName)
				: -1;
			if (isLocationNested()) {
				updatedPathData = splitPaths.slice(startIndex + 1);
			} else updatedPathData = splitPaths.slice(startIndex);
			const updatedData = updatedPathData.map((path) => ({
				kind: 'Folder',
				extension: '',
				name: path
			}));
			return updatedData;
		}
	}, [
		pathInfo,
		isLocationNested,
		pathSlashOS,
		isEphemeralLocation,
		pathNameLocationName,
		fullPathOnClick,
		queriedFullPath.data,
		selectedItem
	]);

	return (
		<div
			className="absolute inset-x-0 bottom-0 flex items-center gap-1 border-t border-t-app-line bg-app/90 px-3.5 text-[11px] text-ink-dull backdrop-blur-lg"
			style={{ height: PATH_BAR_HEIGHT }}
		>
			{data?.map((p, index) => {
				return (
					<Path
						key={(p.name + index).toString()}
						paths={data.map((p) => p.name)}
						path={p}
						index={index}
						fullPathOnClick={fullPathOnClick}
						onClick={() => pathRedirectHandler(p.name, index)}
					/>
				);
			})}
			{selectedItem && (
				<div className="pointer-events-none flex items-center gap-1">
					{data && data.length > 0 && <CaretRight weight="bold" size={10} />}
					<>
						<FileThumb size={16} frame frameClassName="!border" data={selectedItem} />
						{'name' in selectedItem.item ? (
							<span className="max-w-xs truncate text-ink-dull">
								{selectedItem.item.name}
							</span>
						) : (
							<span className="max-w-xs truncate">
								{selectedItem.item.file_paths[0]?.name}
							</span>
						)}
					</>
				</div>
			)}
		</div>
	);
});

interface Props extends ComponentProps<'div'> {
	paths: string[];
	path: {
		name: string;
	};
	fullPathOnClick: boolean;
	index: number;
}

const Path = ({ paths, path, fullPathOnClick, index, ...rest }: Props) => {
	return (
		<div
			className={clsx(
				'flex items-center gap-1',
				fullPathOnClick
					? 'cursor-pointer text-ink-dull'
					: index !== paths.length - 1 && ' cursor-pointer'
			)}
			{...rest}
		>
			<Icon name="Folder" size={16} alt="Folder" />
			<span className="max-w-xs truncate text-ink-dull transition-opacity duration-300 hover:opacity-80">
				{path.name}
			</span>
			{index !== (paths?.length as number) - 1 && (
				<CaretRight weight="bold" className="text-ink-dull" size={10} />
			)}
		</div>
	);
};
