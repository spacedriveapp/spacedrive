import { Circle } from '@phosphor-icons/react';
import clsx from 'clsx';
import { Tag, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Shortcut, toast } from '@sd/ui';
import { useKeybind, useLocale, useOperatingSystem } from '~/hooks';
import { keybindForOs } from '~/util/keybinds';

import { useExplorerContext } from './Context';

export const TAG_BAR_HEIGHT = 64;

// TODO: replace any with proper type here
const toTarget = (data: any) =>
	data &&
	'id' in data.item &&
	(data.type === 'Object'
		? {
				Object: data.item.id
			}
		: {
				FilePath: data.item.id
			});

// million-ignore
// TODO: implement proper custom ordering of tags
export const ExplorerTagBar = (props: {}) => {
	// const [isTagAssignModeActive] = useSelector(explorerStore, (s) => [s.tagAssignMode]);

	const { data: allTags = [] } = useLibraryQuery(['tags.list']);
	const explorer = useExplorerContext();
	const mutation = useLibraryMutation(['tags.assign'], {
		onSuccess: () => {
			// this makes sure that the tags are updated in the UI
			// rspc.queryClient.invalidateQueries(['tags.getForObject'])
			// rspc.queryClient.invalidateQueries(['search.paths'])
			// modalRef.current?.dismiss();
		}
	});

	const { t } = useLocale();

	// This will automagically listen for any keypress 1-9 while the tag bar is visible.
	// These listeners will unmount when ExplorerTagBar is unmounted.
	useKeybind(
		[['Key1'], ['Key2'], ['Key3'], ['Key4'], ['Key5'], ['Key6'], ['Key7'], ['Key8'], ['Key9']],
		async (e) => {
			try {
				await mutation.mutateAsync({
					targets: Array.from(explorer.selectedItems.entries()).map((item) =>
						toTarget(item[0])
					),
					// TODO: remove "!" and do proper conditional run
					tag_id: allTags[+e.key - 1]!.id,
					unassign: false
				});

				toast(`Assigned {tag} to {#} files.`, {
					type: 'success'
				});
			} catch (err) {
				let msg: string = 'An unknown error occurred.';

				if (err instanceof Error) {
					msg = err.message;
					console.error('Tag assignment failed with error', err);
				} else if (typeof err === 'string') {
					msg = err;
				}

				toast(`Could not assign tag {s} to {#} files: ${msg}`, {
					type: 'error'
				});
			}
		}
	);

	return (
		<div
			className={clsx(
				'flex flex-col-reverse items-start border-t border-t-app-line bg-app/90 px-3.5 text-ink-dull backdrop-blur-lg ',
				`h-[${TAG_BAR_HEIGHT}px]`
			)}
		>
			<em>{t('tag_assign_mode_description')}</em>

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

const TagItem = ({ tag, assignKey, onClick }: TagItemProps) => {
	// const isDark = useIsDark();

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
