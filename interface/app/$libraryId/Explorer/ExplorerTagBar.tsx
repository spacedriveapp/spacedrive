import { CaretRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { memo, useMemo } from 'react';
import { useNavigate } from 'react-router';
import { createSearchParams } from 'react-router-dom';
import {
	getExplorerItemData,
	getIndexedItemFilePath,
	useLibraryQuery,
	useSelector
} from '@sd/client';
import { Icon } from '~/components';
import { useIsDark, useOperatingSystem } from '~/hooks';

import { useExplorerContext } from './Context';
import { FileThumb } from './FilePath/Thumb';
import { explorerStore } from './store';
import { useExplorerDroppable } from './useExplorerDroppable';
import { useExplorerSearchParams } from './util';

export const PATH_BAR_HEIGHT = 32;

export const ExplorerTagBar = memo(() => {
	const [isTagAssignModeActive] = useSelector(explorerStore, (s) => [s.tagAssignMode]);

	return (
		<div
			className="flex items-center border-t border-t-app-line bg-app/90 px-3.5 text-ink-dull backdrop-blur-lg"
			style={{ height: PATH_BAR_HEIGHT }}
		>
			{/* {paths.map((path) => (
				<Path
					key={path.pathname}
					path={path}
					onClick={() => handleOnClick(path)}
					disabled={path.pathname === (searchPath ?? (location && '/'))}
				/>
			))} */}

			{/* {selectedItem && (!queryPath || filePathname) && (
				<div className="ml-1 flex items-center gap-1">
					<FileThumb data={selectedItem} size={16} frame frameClassName="!border" />
					<span className="max-w-xs truncate">
						{getExplorerItemData(selectedItem).fullName}
					</span>
				</div>
			)} */}
		</div>
	);
});

interface TagItemProps {
	tag: { tagId: number; name: string; pathname: string };
	onClick: () => void;
	disabled: boolean;
}

const TagItem = ({ tag, onClick, disabled }: TagItemProps) => {
	const isDark = useIsDark();

	// const { setDroppableRef, className, isDroppable } = useExplorerDroppable({
	// 	data: {
	// 		type: 'tag',
	// 		path: tag.pathname,
	// 		// data: { id: tag.tagId , }
	// 		data: undefined
	// 	},
	// 	allow: ['Path', 'NonIndexedPath', 'Object'],
	// 	navigateTo: onClick,
	// 	disabled
	// });

	return (
		<button
			// ref={setDroppableRef}
			className={clsx(
				'group flex items-center gap-1 rounded px-1 py-0.5',
				// isDroppable && [isDark ? 'bg-app-button/70' : 'bg-app-darkerBox'],
				!disabled && [isDark ? 'hover:bg-app-button/70' : 'hover:bg-app-darkerBox']
				// className
			)}
			disabled={disabled}
			onClick={onClick}
			tabIndex={-1}
		>
			<Icon name="Folder" size={16} alt="Folder" />
			<span className="max-w-xs truncate text-ink-dull">{tag.name}</span>
			<CaretRight weight="bold" className="text-ink-dull group-last:hidden" size={10} />
		</button>
	);
};
