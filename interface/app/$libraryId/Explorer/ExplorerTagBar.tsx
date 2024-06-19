import { Circle } from '@phosphor-icons/react';
import clsx from 'clsx';
import { ReactNode, useCallback, useEffect, useRef, useState } from 'react';
import {
	ExplorerItem,
	Tag,
	Target,
	useLibraryMutation,
	useLibraryQuery,
	useRspcContext,
	useSelector
} from '@sd/client';
import { Shortcut, toast } from '@sd/ui';
import { useIsDark, useKeybind, useLocale, useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

import { useExplorerContext } from './Context';
import { explorerStore } from './store';

export const TAG_BAR_HEIGHT = 64;
const NUMBER_KEYCODES: string[][] = [
	['Key1'],
	['Key2'],
	['Key3'],
	['Key4'],
	['Key5'],
	['Key6'],
	['Key7'],
	['Key8'],
	['Key9']
];

// TODO: hoist this to somewhere higher as a utility function
const toTarget = (data: ExplorerItem): Target => {
	if (!data || !('id' in data.item))
		throw new Error('Tried to convert an invalid object to Target.');

	return (
		data.type === 'Object'
			? {
					Object: data.item.id
				}
			: {
					FilePath: data.item.id
				}
	) satisfies Target;
};

type TagBulkAssignHotkeys = typeof explorerStore.tagBulkAssignHotkeys;
function getHotkeysWithNewAssignment(
	hotkeys: TagBulkAssignHotkeys,
	options:
		| {
				unassign?: false;
				tagId: number;
				hotkey: string;
		  }
		| {
				unassign: true;
				tagId: number;
				hotkey?: string;
		  }
): TagBulkAssignHotkeys {
	const hotkeysWithoutCurrentTag = hotkeys.filter(
		({ hotkey, tagId }) => !(tagId === options.tagId || hotkey === options.hotkey)
	);

	if (options.unassign) {
		return hotkeysWithoutCurrentTag;
	}

	return hotkeysWithoutCurrentTag.concat({
		hotkey: options.hotkey,
		tagId: options.tagId
	});
}

// million-ignore
export const ExplorerTagBar = () => {
	const [tagBulkAssignHotkeys] = useSelector(explorerStore, (s) => [s.tagBulkAssignHotkeys]);
	const explorer = useExplorerContext();
	const rspc = useRspcContext();
	const tagsRef = useRef<HTMLUListElement | null>(null);
	const [isTagsOverflowing, setIsTagsOverflowing] = useState(false);

	const updateOverflowState = () => {
		const element = tagsRef.current;
		if (element) {
			setIsTagsOverflowing(
				element.scrollHeight > element.clientHeight || element.scrollWidth > element.clientWidth
			);
		}
	}

	useEffect(() => {
		const element = tagsRef.current;
		if (!element) return;
		//handles initial render when not resizing
		setIsTagsOverflowing(element.scrollHeight > element.clientHeight || element.scrollWidth > element.clientWidth)
		//make sure state updates when window resizing
		window.addEventListener('resize', () => {
			updateOverflowState();
		})
		//remove listeners on unmount
		return () => {
			window.removeEventListener('resize', () => {
				updateOverflowState();
			})
		}
	}, [tagsRef])

	const [tagListeningForKeyPress, setTagListeningForKeyPress] = useState<number | undefined>();

	const { data: allTags = [] } = useLibraryQuery(['tags.list']);
	const mutation = useLibraryMutation(['tags.assign'], {
		onSuccess: () => rspc.queryClient.invalidateQueries(['search.paths'])
	});

	const { t } = useLocale();

	// This will automagically listen for any keypress 1-9 while the tag bar is visible.
	// These listeners will unmount when ExplorerTagBar is unmounted.
	useKeybind(
		NUMBER_KEYCODES,
		async (e) => {
			const targets = Array.from(explorer.selectedItems.entries()).map((item) =>
				toTarget(item[0])
			);

			// Silent fail if no files are selected
			if (targets.length < 1) return;

			const keyPressed = e.key;

			let tag: Tag | undefined;

			findTag: {
				const tagId = tagBulkAssignHotkeys.find(
					({ hotkey }) => hotkey === keyPressed
				)?.tagId;
				const foundTag = allTags.find((t) => t.id === tagId);

				if (!foundTag) break findTag;

				tag = foundTag;
			}

			if (!tag) return;

			try {
				await mutation.mutateAsync({
					targets,
					tag_id: tag.id,
					unassign: false
				});

				toast(
					t('tags_bulk_assigned', {
						tag_name: tag.name,
						file_count: targets.length
					}),
					{
						type: 'success'
					}
				);
			} catch (err) {
				let msg: string = t('error_unknown');

				if (err instanceof Error || (typeof err === 'object' && err && 'message' in err)) {
					msg = err.message as string;
				} else if (typeof err === 'string') {
					msg = err;
				}

				console.error('Tag assignment failed with error', err);

				let failedToastMessage: string = t('tags_bulk_failed_without_tag', {
					file_count: targets.length,
					error_message: msg
				});

				if (tag)
					failedToastMessage = t('tags_bulk_failed_with_tag', {
						tag_name: tag.name,
						file_count: targets.length,
						error_message: msg
					});

				toast(failedToastMessage, {
					type: 'error'
				});
			}
		},
		{
			enabled: typeof tagListeningForKeyPress !== 'number'
		}
	);

	return (
		<div
			className={clsx(
				'flex flex-row flex-wrap-reverse items-center justify-between gap-1 border-t border-t-app-line bg-app/90 px-3.5 py-2 text-ink-dull backdrop-blur-lg',
			)}
		>
			<em className="text-sm tracking-wide">{t('tags_bulk_instructions')}</em>

			<ul
				ref={tagsRef}
				// TODO: I want to replace this `overlay-scroll` style with a better system for non-horizontral-scroll mouse users, but
				// for now this style looks the least disgusting. Probably will end up going for a left/right arrows button that dynamically
				// shows/hides depending on scroll position.
				className={clsx(
					'flex-0 explorer-scroll my-1 flex max-w-full list-none flex-row gap-2 overflow-x-auto',
					isTagsOverflowing ? 'pb-2' : 'pb-0'
				)}
			>
				{/* Did not want to write a .toSorted() predicate for this so lazy spreading things with hotkeys first then the rest after */}
				{allTags
					.toSorted((tagA, tagB) => {
						// Sort this array by hotkeys 1-9 first, then unasssigned tags. I know, it's terrible.
						// This 998/999 bit is likely terrible for sorting. I'm bad at writing sort predicates.
						// Improvements (probably much simpler than this anyway) are much welcome <3
						// -- iLynxcat 3/jun/2024

						const hotkeyA = +(
							tagBulkAssignHotkeys.find((k) => k.tagId === tagA.id)?.hotkey ?? 998
						);
						const hotkeyB = +(
							tagBulkAssignHotkeys.find((k) => k.tagId === tagB.id)?.hotkey ?? 999
						);

						return hotkeyA - hotkeyB;
					})
					.map((tag) => (
						<li key={tag.id}>
							<TagItem
								tag={tag}
								assignKey={
									tagBulkAssignHotkeys.find(({ tagId }) => tagId === tag.id)
										?.hotkey
								}
								isAwaitingKeyPress={tagListeningForKeyPress === tag.id}
								onClick={() => {
									setTagListeningForKeyPress(tag.id);
								}}
								onKeyPress={(e) => {
									if (e.key === 'Escape') {
										explorerStore.tagBulkAssignHotkeys =
											getHotkeysWithNewAssignment(tagBulkAssignHotkeys, {
												unassign: true,
												tagId: tag.id
											});

										setTagListeningForKeyPress(undefined);

										return;
									}

									explorerStore.tagBulkAssignHotkeys =
										getHotkeysWithNewAssignment(tagBulkAssignHotkeys, {
											tagId: tag.id,
											hotkey: e.key
										});
									setTagListeningForKeyPress(undefined);
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
	assignKey?: string;
	isAwaitingKeyPress: boolean;
	onKeyPress: (e: KeyboardEvent) => void;
	onClick: () => void;
}

const TagItem = ({
	tag,
	assignKey,
	isAwaitingKeyPress = false,
	onKeyPress,
	onClick
}: TagItemProps) => {
	const buttonRef = useRef<HTMLButtonElement>(null);
	const isDark = useIsDark();

	const os = useOperatingSystem(true);
	const keybind = keybindForOs(os);

	useKeybind(
		[...NUMBER_KEYCODES, ['Escape']],
		(e) => {
			buttonRef.current?.blur(); // Hides the focus ring after Escape is pressed to cancel assignment
			return onKeyPress(e);
		},
		{
			enabled: isAwaitingKeyPress
		}
	);

	return (
		<button
			className={clsx('group flex items-center gap-1 rounded-lg border px-2.5 py-0.5', {
				['border border-app-line bg-app-box']: !isAwaitingKeyPress && isDark,
				['border-accent bg-app-box']: isAwaitingKeyPress && isDark,
				['border-accent bg-app-lightBox']: isAwaitingKeyPress && !isDark
			})}
			ref={buttonRef}
			onClick={onClick}
			aria-live={isAwaitingKeyPress ? 'assertive' : 'off'}
			aria-label={
				isAwaitingKeyPress
					? `Type a number to map it to the "${tag.name}" tag. Press escape to cancel.`
					: undefined
			}
		>
			<Circle
				fill={tag.color ?? 'grey'}
				weight="fill"
				alt=""
				aria-hidden
				className="size-2.5"
			/>
			<span className="max-w-xs truncate py-0.5 text-sm font-semibold text-ink-dull">
				{tag.name}
			</span>

			{assignKey && <Shortcut chars={keybind([], [assignKey])} />}
		</button>
	);
};
