import { Circle } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useRef, useState } from 'react';
import {
	ExplorerItem,
	Tag,
	Target,
	useLibraryMutation,
	useLibraryQuery,
	useSelector
} from '@sd/client';
import { Shortcut, toast } from '@sd/ui';
import { useIsDark, useKeybind, useLocale, useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

import { useExplorerContext } from './Context';
import { explorerStore } from './store';

export const TAG_BAR_HEIGHT = 128;
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

// million-ignore
export const ExplorerTagBar = (props: {}) => {
	const [tagBulkAssignHotkeys] = useSelector(explorerStore, (s) => [s.tagBulkAssignHotkeys]);
	const explorer = useExplorerContext();

	const [tagListeningForKeyPress, setTagListeningForKeyPress] = useState<number | undefined>();

	const { data: allTags = [] } = useLibraryQuery(['tags.list']);
	const mutation = useLibraryMutation(['tags.assign']);

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
				'flex flex-col-reverse content-center items-start border-t border-t-app-line bg-app/90 px-3.5 py-1 text-ink-dull backdrop-blur-lg ',
				`h-[${TAG_BAR_HEIGHT}px]`
			)}
		>
			<em className="text-sm tracking-wide">{t('tags_bulk_instructions')}</em>

			<ul className={clsx('flex list-none flex-row gap-2')}>
				{allTags.map((tag, i) => (
					<li key={tag.id}>
						<TagItem
							tag={tag}
							assignKey={
								tagBulkAssignHotkeys.find(({ hotkey, tagId }) => tagId === tag.id)
									?.hotkey
							}
							isAwaitingKeyPress={tagListeningForKeyPress === tag.id}
							onClick={() => {
								setTagListeningForKeyPress(tag.id);
							}}
							onKeyPress={(e) => {
								if (e.key === 'Escape') {
									setTagListeningForKeyPress(undefined);
									return void console.log(
										'Tag hotkey assignment cancelled via escape key'
									);
								}

								explorerStore.tagBulkAssignHotkeys =
									explorerStore.tagBulkAssignHotkeys
										.filter(
											({ hotkey, tagId }) =>
												hotkey !== e.key && tagId !== tag.id
										)
										.concat({
											hotkey: e.key,
											tagId: tag.id
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
			buttonRef.current?.blur();
			return onKeyPress(e);
		},
		{
			enabled: isAwaitingKeyPress
		}
	);

	return (
		<button
			className={clsx('group flex items-center gap-1 rounded-lg border px-1 py-0.5', {
				['border-gray-500 bg-gray-500']: isDark,
				['border-blue-500 bg-blue-700']: isAwaitingKeyPress && isDark,
				['border-blue-500 bg-blue-200']: isAwaitingKeyPress && !isDark
			})}
			ref={buttonRef}
			onClick={onClick}
			tabIndex={-1}
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
				className="size-3"
			/>
			<span className="max-w-xs truncate text-sm font-semibold text-ink-dull">
				{tag.name}
			</span>

			{assignKey && <Shortcut chars={keybind([], [assignKey])} />}
		</button>
	);
};
