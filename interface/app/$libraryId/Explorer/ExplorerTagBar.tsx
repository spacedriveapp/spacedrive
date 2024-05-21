import { Circle } from '@phosphor-icons/react';
import clsx from 'clsx';
import { KeyboardEventHandler } from 'react';
import { Tag, useCache, useLibraryQuery, useNodes, useSelector } from '@sd/client';
import { Shortcut, toast } from '@sd/ui';
import { useIsDark, useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

import { explorerStore } from './store';

export const TAG_BAR_HEIGHT = 64;

// Capture the next keypress in the window.
// "greedy" because we entirely cancel the keypress and intercept it for our own purposes.
// TODO: remove if tag assign mode changes as well and reflect that in ui.

function captureTagAssignKeyPress(handler: KeyboardEventHandler): void {
	toast.info('Capturing keyup...');

	function handleKeyPress(event: KeyboardEvent) {
		// toast.success('Captured: ' + event.key);
		window.removeEventListener('keypress', handleKeyPress);

		event.preventDefault();
		event.stopPropagation();
	}

	window.addEventListener('keypress', handleKeyPress);
}

export const ExplorerTagBar = () => {
	const [isTagAssignModeActive, awaitingKeyPress] = useSelector(explorerStore, (s) => [
		s.tagAssignMode,
		s.awaitingTagAssignKeypress
	]);

	const allTagsQuery = useLibraryQuery(['tags.list']);

	useNodes(allTagsQuery.data?.nodes);
	const tagData = useCache(allTagsQuery.data?.items);

	const availableTags = tagData;

	return (
		<div
			className={clsx(
				'flex flex-col-reverse items-start border-t border-t-app-line bg-app/90 px-3.5 text-ink-dull backdrop-blur-lg ',
				`h-[${TAG_BAR_HEIGHT}px]`
			)}
		>
			{/* not final ui/copy, want to give some kind of on-demand help for tag assign mode. */}
			<em className={clsx('line-clamp-1 text-sm tracking-wide')}>
				{JSON.stringify(availableTags)}
			</em>

			<ul className={clsx('flex list-none flex-row gap-2')}>
				{availableTags.map((tag, i) => {
					console.log(++i);

					return (
						<li key={tag.id}>
							<TagItem
								tag={tag}
								assignKey={(++i).toString()}
								onClick={() => {
									// greedyCaptureNextKeyPress()
									// 	.then()
									// 	.catch((e) => {
									// 		toast.error('Failed to capture keypress', e);
									// 	});
								}}
							/>
						</li>
					);
				})}
			</ul>
		</div>
	);
};

interface TagItemProps {
	tag: Tag;
	assignKey: string;
	onClick: () => void;
}

const TagItem = ({ tag, assignKey, onClick }: TagItemProps) => {
	const isDark = useIsDark();

	const os = useOperatingSystem(true);
	const keybind = keybindForOs(os);

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
				'group flex items-center gap-1 rounded-lg border border-gray-500 bg-gray-500 px-1 py-0.5'
			)}
			onClick={onClick}
			tabIndex={-1}
		>
			<Circle fill={tag.color ?? 'grey'} weight="fill" alt="" className="size-3" />
			<span className="max-w-xs truncate text-ink-dull">{tag.name}</span>

			<Shortcut chars={keybind([], [assignKey])} />
		</button>
	);
};
