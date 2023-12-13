import { CaretRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { memo, useMemo } from 'react';
import { useNavigate } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import { getExplorerItemData, getIndexedItemFilePath, useLibraryQuery } from '@sd/client';
import { Icon } from '~/components';
import { useIsDark, useOperatingSystem } from '~/hooks';

import { useExplorerContext } from './Context';
import { FileThumb } from './FilePath/Thumb';
import { useExplorerDroppable } from './useExplorerDroppable';
import { useExplorerSearchParams } from './util';

export const PATH_BAR_HEIGHT = 32;

export const ExplorerPath = memo(() => {
	const os = useOperatingSystem();
	const navigate = useNavigate();

	const [{ path: searchPath }] = useExplorerSearchParams();
	const { parent: explorerParent, selectedItems } = useExplorerContext();

	const pathSlash = os === 'windows' ? '\\' : '/';

	const location = explorerParent?.type === 'Location' ? explorerParent.location : undefined;

	const selectedItem = useMemo(
		() => (selectedItems.size === 1 ? [...selectedItems][0] : undefined),
		[selectedItems]
	);

	const indexedFilePath = selectedItem && getIndexedItemFilePath(selectedItem);

	const queryPath = !!indexedFilePath && (!searchPath || !location);

	const { data: filePathname } = useLibraryQuery(['files.getPath', indexedFilePath?.id ?? -1], {
		enabled: queryPath
	});

	const paths = useMemo(() => {
		// Remove file name from the path
		const _filePathname = filePathname?.slice(0, filePathname.lastIndexOf(pathSlash));

		const pathname = _filePathname ?? [location?.path, searchPath].filter(Boolean).join('');

		const paths = [...(pathname.match(new RegExp(`[^${pathSlash}]+`, 'g')) ?? [])];

		let locationPath = location?.path;

		if (!locationPath && indexedFilePath?.materialized_path) {
			if (indexedFilePath.materialized_path === '/') locationPath = pathname;
			else {
				// Remove last slash from materialized_path
				const materializedPath = indexedFilePath.materialized_path.slice(0, -1);

				// Extract location path from pathname
				locationPath = pathname.slice(0, pathname.indexOf(materializedPath));
			}
		}

		const locationIndex = (locationPath ?? '').split(pathSlash).filter(Boolean).length - 1;

		return paths.map((path, i) => {
			const isLocation = locationIndex !== -1 && i >= locationIndex;

			const _paths = [
				...paths.slice(!isLocation ? 0 : locationIndex + 1, i),
				i === locationIndex ? '' : path
			];

			let pathname = `${pathSlash}${_paths.join(pathSlash)}`;

			// Add slash to the end of the pathname if it's a location
			if (isLocation && i > locationIndex) pathname += pathSlash;

			return {
				name: path,
				pathname,
				locationId: isLocation ? indexedFilePath?.location_id ?? location?.id : undefined
			};
		});
	}, [location, indexedFilePath, filePathname, pathSlash, searchPath]);

	const handleOnClick = ({ pathname, locationId }: (typeof paths)[number]) => {
		if (locationId === undefined) {
			// TODO: Handle ephemeral volumes
			navigate({
				pathname: '../ephemeral/0-0',
				search: `${createSearchParams({ path: pathname })}`
			});
		} else {
			navigate({
				pathname: `../location/${locationId}`,
				search: pathname === '/' ? undefined : `${createSearchParams({ path: pathname })}`
			});
		}
	};

	return (
		<div
			className="group absolute inset-x-0 bottom-0 z-50 flex items-center border-t border-t-app-line bg-app/90 px-3.5 text-[11px] text-ink-dull backdrop-blur-lg"
			style={{ height: PATH_BAR_HEIGHT }}
		>
			{paths.map((path) => (
				<Path
					key={path.pathname}
					path={path}
					onClick={() => handleOnClick(path)}
					disabled={path.pathname === (searchPath ?? (location && '/'))}
				/>
			))}

			{selectedItem && (!queryPath || filePathname) && (
				<div className="ml-1 flex items-center gap-1">
					<FileThumb data={selectedItem} size={16} frame frameClassName="!border" />
					<span className="max-w-xs truncate">
						{getExplorerItemData(selectedItem).fullName}
					</span>
				</div>
			)}
		</div>
	);
});

interface PathProps {
	path: { name: string; pathname: string; locationId?: number };
	onClick: () => void;
	disabled: boolean;
}

const Path = ({ path, onClick, disabled }: PathProps) => {
	const isDark = useIsDark();

	const { setDroppableRef, className, isDroppable } = useExplorerDroppable({
		data: {
			type: 'location',
			path: path.pathname,
			data: path.locationId ? { id: path.locationId, path: path.pathname } : undefined
		},
		allow: ['Path', 'NonIndexedPath', 'Object'],
		navigateTo: onClick,
		disabled
	});

	return (
		<button
			ref={setDroppableRef}
			className={clsx(
				'group flex items-center gap-1 rounded px-1 py-0.5',
				isDroppable && [isDark ? 'bg-app-lightBox' : 'bg-app-darkerBox'],
				!disabled && [isDark ? 'hover:bg-app-lightBox' : 'hover:bg-app-darkerBox'],
				className
			)}
			disabled={disabled}
			onClick={onClick}
			tabIndex={-1}
		>
			<Icon name="Folder" size={16} alt="Folder" />
			<span className="max-w-xs truncate text-ink-dull">{path.name}</span>
			<CaretRight weight="bold" className="text-ink-dull group-last:hidden" size={10} />
		</button>
	);
};
