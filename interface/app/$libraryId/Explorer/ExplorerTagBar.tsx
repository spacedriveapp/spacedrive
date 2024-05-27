import clsx from 'clsx';
import { KeyboardEventHandler } from 'react';
import { Tag, useLibraryQuery, useSelector } from '@sd/client';
import { toast } from '@sd/ui';
import { useKeybind } from '~/hooks';

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

// @million-ignore
export const ExplorerTagBar = () => {
	const [isTagAssignModeActive, awaitingKeyPress] = useSelector(explorerStore, (s) => [
		s.tagAssignMode,
		s.awaitingTagAssignKeypress
	]);

	const { data: allTags = [] } = useLibraryQuery(['tags.list']);

	// This will automagically listen for any keypress 1-9 while the tag bar is visible.
	// These listeners will unmount when ExplorerTagBar is unmounted.
	useKeybind(
		[['Key1'], ['Key2'], ['Key3'], ['Key4'], ['Key5'], ['Key6'], ['Key7'], ['Key8'], ['Key9']],
		(e) => {
			// TODO: actually do tag assignment once pressed
		}
	);

	return (
		<div
			className={clsx(
				'flex flex-col-reverse items-start border-t border-t-app-line bg-app/90 px-3.5 text-ink-dull backdrop-blur-lg ',
				`h-[${TAG_BAR_HEIGHT}px]`
			)}
		>
			<em>{JSON.stringify(allTags)}</em>

			<ul className={clsx('flex list-none flex-row gap-2')}>
				{allTags.map((tag, i) => (
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
				))}
			</ul>
		</div>
	);
};

interface TagItemProps {
	tag: Tag;
	assignKey: string;
	onClick: () => void;
}

// @million-ignore
const TagItem = ({ tag, assignKey, onClick }: TagItemProps) => {
	// const isDark = useIsDark();

	// const os = useOperatingSystem(true);
	// const keybind = keybindForOs(os);

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
		<span className="max-w-xs truncate text-ink-dull">{tag.name}</span>
		// <button
		// ref={setDroppableRef}
		// className={clsx(
		// 	'group flex items-center gap-1 rounded-lg border border-gray-500 bg-gray-500 px-1 py-0.5'
		// )}
		// onClick={onClick}
		// tabIndex={-1}
		// >
		// {/* <Circle fill={tag.color ?? 'grey'} weight="fill" alt="" className="size-3" /> */}

		// {/* <Shortcut chars={keybind([], [assignKey])} /> */}
		// {/* </button> */}
	);
};
