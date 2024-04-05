import { AppWindow, CaretRight, ClipboardText } from '@phosphor-icons/react';
import clsx from 'clsx';
import { memo, useMemo, useState } from 'react';
import { useNavigate } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import {
	getExplorerItemData,
	getIndexedItemFilePath,
	useLibraryContext,
	useLibraryQuery
} from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { Icon } from '~/components';
import { useIsDark, useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { useExplorerContext } from './Context';
import { FileThumb } from './FilePath/Thumb';
import { useExplorerDroppable } from './useExplorerDroppable';
import { useExplorerSearchParams } from './util';

export const PATH_BAR_HEIGHT = 32;

export const ExplorerPath = memo(() => {
	const os = useOperatingSystem(true);
	const navigate = useNavigate();
	const [{ path: searchPath }] = useExplorerSearchParams();
	const { parent: explorerParent, selectedItems } = useExplorerContext();

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
		const pathSlash = os === 'windows' ? '\\' : '/';

		// Replace all slashes with native slashes
		// TODO: Fix returned path from query on windows as the location part of the path
		// uses "/" instead of "\" -> C:\Users\sd-user\Documents\spacedrive\packages/assets/deps
		let _filePathname = filePathname?.replaceAll(/[\\/]/g, pathSlash);

		// Remove file name from the path
		_filePathname = _filePathname?.slice(0, _filePathname.lastIndexOf(pathSlash) + 1);

		const pathname = _filePathname ?? [location?.path, searchPath].filter(Boolean).join('');

		const paths = [...(pathname.match(/[^\\/]+/g) ?? [])];

		let locationPath = location?.path;

		if (!locationPath && indexedFilePath?.materialized_path) {
			if (indexedFilePath.materialized_path === '/') locationPath = pathname;
			else {
				let materializedPath = indexedFilePath.materialized_path;

				// Replace all slashes with native slashes
				if (os === 'windows') materializedPath = materializedPath.replaceAll('/', '\\');

				// Extract location path from pathname
				locationPath = pathname.slice(0, pathname.lastIndexOf(materializedPath));
			}
		}

		const locationIndex = (locationPath ?? '').split(pathSlash).filter(Boolean).length - 1;

		return paths.map((path, i) => {
			const isLocation = locationIndex !== -1 && i >= locationIndex;

			const _paths = [
				...paths.slice(!isLocation ? 0 : locationIndex + 1, i),
				i === locationIndex ? '' : path
			];

			let pathname = _paths.join(isLocation ? '/' : pathSlash);

			// Wrap pathname in slashes if it's a location
			if (isLocation) pathname = pathname ? `/${pathname}/` : '/';
			// Add slash to the end of the pathname if it's the root of a drive on windows (C: -> C:\)
			else if (os === 'windows' && _paths.length === 1) pathname += pathSlash;
			// Add slash to the beginning of the ephemeral pathname (Users -> /Users)
			else if (os !== 'windows') pathname = `/${pathname}`;

			return {
				name: path,
				pathname,
				locationId: isLocation ? indexedFilePath?.location_id ?? location?.id : undefined
			};
		});
	}, [location, indexedFilePath, filePathname, searchPath, os]);

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
	const { revealItems } = usePlatform();
	const { library } = useLibraryContext();
	const [contextMenuOpen, setContextMenuOpen] = useState(false);
	const os = useOperatingSystem(true);
	const isSlashAtEnd = path.pathname.endsWith(os == 'windows' ? '\\' : '/'); // Checks if the path is ephemeral or not
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
		<ContextMenu.Root
			onOpenChange={setContextMenuOpen}
			trigger={
				<button
					ref={setDroppableRef}
					className={clsx(
						'group flex items-center gap-1 rounded px-1 py-0.5',
						(isDroppable || contextMenuOpen) && [
							isDark ? 'bg-app-button/70' : 'bg-app-darkerBox'
						],
						!disabled && [isDark ? 'hover:bg-app-button/70' : 'hover:bg-app-darkerBox'],
						className
					)}
					disabled={disabled}
					onClick={onClick}
					tabIndex={-1}
				>
					<Icon name="Folder" size={16} alt="Folder" />
					<span className="max-w-xs truncate text-ink-dull">{path.name}</span>
					<CaretRight
						weight="bold"
						className="text-ink-dull group-last:hidden"
						size={10}
					/>
				</button>
			}
		>
			<ContextMenu.Item
				onClick={() => {
					if (!revealItems) return null;
					revealItems(library.uuid, [
						isSlashAtEnd
							? {
									Location: { id: path.locationId! }
								}
							: {
									Ephemeral: { path: path.pathname }
								}
					]);
				}}
				label="Open in Finder"
				icon={AppWindow}
			/>
			<ContextMenu.Item
				onClick={() => navigator.clipboard.writeText(path.pathname)}
				icon={ClipboardText}
				label={`Copy "${path.name}" as path`}
			/>
		</ContextMenu.Root>
	);
};
