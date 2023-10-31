import { Plus } from '@phosphor-icons/react';
import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { useRef } from 'react';
import {
	ExplorerItem,
	libraryClient,
	useLibraryMutation,
	useLibraryQuery,
	usePlausibleEvent
} from '@sd/client';
import { dialogManager, ModifierKeys } from '@sd/ui';
import CreateDialog, {
	assignItemsToTag,
	AssignTagItems
} from '~/app/$libraryId/settings/library/tags/CreateDialog';
import { Menu } from '~/components/Menu';
import { useOperatingSystem } from '~/hooks';
import { useScrolled } from '~/hooks/useScrolled';
import { keybindForOs } from '~/util/keybinds';

export default (props: { items: Array<Extract<ExplorerItem, { type: 'Object' | 'Path' }>> }) => {
	const os = useOperatingSystem();
	const keybind = keybindForOs(os);
	const submitPlausibleEvent = usePlausibleEvent();
	const tags = useLibraryQuery(['tags.list'], { suspense: true });

	// Map<tag::id, Vec<object::id>>
	const tagsWithObjects = useLibraryQuery([
		'tags.getWithObjects',
		props.items
			.map((item) => {
				if (item.type === 'Path') return item.item.object?.id;
				else if (item.type === 'Object') return item.item.id;
			})
			.filter((item): item is number => item !== undefined)
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

	return (
		<>
			<Menu.Item
				className="tag-menu"
				label="New tag"
				icon={Plus}
				iconProps={{ size: 15 }}
				keybind={keybind([ModifierKeys.Control], ['N'])}
				onClick={() => {
					dialogManager.create((dp) => <CreateDialog {...dp} items={props.items} />);
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

							const objectsWithTag = new Set(tagsWithObjects.data?.[tag?.id]);

							// only unassign if all objects have tag
							// this is the same functionality as finder
							const unassign = props.items.every((item) => {
								if (item.type === 'Object') {
									return objectsWithTag.has(item.item.id);
								} else {
									const { object } = item.item;

									if (!object) return false;
									return objectsWithTag.has(object.id);
								}
							});

							// TODO: UI to differentiate tag assigning when some objects have tag when no objects have tag - ENG-965

							return (
								<Menu.Item
									key={virtualRow.index}
									className="absolute left-0 top-0 w-full"
									style={{
										height: `${virtualRow.size}px`,
										transform: `translateY(${virtualRow.start}px)`
									}}
									onClick={async (e) => {
										e.preventDefault();

										await assignItemsToTag(
											libraryClient,
											tag.id,
											unassign
												? // use objects that already have tag
												  props.items.flatMap((item) => {
														if (
															item.type === 'Object' ||
															item.type === 'Path'
														) {
															return [item];
														}

														return [];
												  })
												: // use objects that don't have tag
												  props.items.flatMap<AssignTagItems[number]>(
														(item) => {
															if (item.type === 'Object') {
																if (
																	!objectsWithTag.has(
																		item.item.id
																	)
																)
																	return [item];
															} else if (item.type === 'Path') {
																return [item];
															}

															return [];
														}
												  ),
											unassign
										);
									}}
								>
									<div
										className="mr-0.5 h-[15px] w-[15px] shrink-0 rounded-full border"
										style={{
											backgroundColor:
												objectsWithTag &&
												objectsWithTag.size > 0 &&
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
