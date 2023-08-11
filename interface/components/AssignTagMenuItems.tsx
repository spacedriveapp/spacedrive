import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { Plus } from 'phosphor-react';
import { useRef } from 'react';
import { Object, useLibraryMutation, useLibraryQuery, usePlausibleEvent } from '@sd/client';
import {
	ContextMenu,
	DropdownMenu,
	ModifierKeys,
	dialogManager,
	useContextMenu,
	useDropdownMenu
} from '@sd/ui';
import CreateDialog from '~/app/$libraryId/settings/library/tags/CreateDialog';
import { useOperatingSystem } from '~/hooks';
import { useScrolled } from '~/hooks/useScrolled';
import { keybindForOs } from '~/util/keybinds';

export default (props: { objects: Object[] }) => {
	const os = useOperatingSystem();
	const keybind = keybindForOs(os);
	const submitPlausibleEvent = usePlausibleEvent();

	const tags = useLibraryQuery(['tags.list'], { suspense: true });
	// Map<tag::id, Vec<object::id>>
	const tagsWithObjects = useLibraryQuery([
		'tags.getWithObjects',
		props.objects.map(({ id }) => id)
	]);

	const assignTag = useLibraryMutation('tags.assign', {
		onSuccess: () => {
			submitPlausibleEvent({ event: { type: 'tagAssign' } });
		}
	});

	const parentRef = useRef<HTMLDivElement>(null);
	const rowVirtualizer = useVirtualizer({
		count: tags.data?.length || 0,
		getScrollElement: () => parentRef.current,
		estimateSize: () => 30,
		paddingStart: 2
	});

	const { isScrolled } = useScrolled(parentRef, 10);

	const isDropdownMenu = useDropdownMenu();
	const isContextMenu = useContextMenu();
	const Menu = isDropdownMenu ? DropdownMenu : isContextMenu ? ContextMenu : undefined;

	if (!Menu) return null;
	return (
		<>
			<Menu.Item
				label="New tag"
				icon={Plus}
				iconProps={{ size: 15 }}
				keybind={keybind([ModifierKeys.Control], ['N'])}
				onClick={() => {
					dialogManager.create((dp) => <CreateDialog {...dp} objects={props.objects} />);
				}}
			/>
			<Menu.Separator className={clsx('mx-0 mb-0 transition', isScrolled && 'shadow')} />
			{tags.data && tags.data.length > 0 ? (
				<div
					ref={parentRef}
					style={{
						maxHeight: `400px`,
						height: `100%`,
						width: `100%`,
						overflow: 'auto'
					}}
				>
					<div
						style={{
							height: `${rowVirtualizer.getTotalSize()}px`,
							width: '100%',
							position: 'relative'
						}}
					>
						{rowVirtualizer.getVirtualItems().map((virtualRow) => {
							const tag = tags.data[virtualRow.index];
							if (!tag) return null;

							const objectsWithTag = tagsWithObjects.data?.[tag?.id];

							// only unassign if all objects have tag
							// this is the same functionality as finder
							const unassign = objectsWithTag?.length === props.objects.length;

							// TODO: UI to differentiate tag assigning when some objects have tag when no objects have tag - ENG-965

							return (
								<Menu.Item
									key={virtualRow.index}
									style={{
										position: 'absolute',
										top: 0,
										left: 0,
										width: '100%',
										height: `${virtualRow.size}px`,
										transform: `translateY(${virtualRow.start}px)`
									}}
									onClick={async (e) => {
										e.preventDefault();

										await assignTag.mutateAsync({
											unassign,
											tag_id: tag.id,
											object_ids: unassign
												? // use objects that already have tag
												  objectsWithTag
												: // use objects that don't have tag
												  props.objects
														.filter(
															(o) =>
																!objectsWithTag?.some(
																	(ot) => ot === o.id
																)
														)
														.map((o) => o.id)
										});
									}}
								>
									<div
										className="mr-0.5 h-[15px] w-[15px] shrink-0 rounded-full border"
										style={{
											backgroundColor:
												objectsWithTag &&
												objectsWithTag.length > 0 &&
												tag.color
													? tag.color
													: 'transparent',
											borderColor: tag.color || '#efefef'
										}}
									/>
									<span className="truncate">{tag.name}</span>
								</Menu.Item>
							);
						})}
					</div>
				</div>
			) : (
				<div className="py-1 text-center text-xs text-ink-faint">
					{tags.data ? 'No tags' : 'Failed to load tags'}
				</div>
			)}
		</>
	);
};
