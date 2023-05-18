import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { Plus } from 'phosphor-react';
import { useRef } from 'react';
import { useLibraryMutation, useLibraryQuery, usePlausibleEvent } from '@sd/client';
import { ContextMenu, DropdownMenu, dialogManager, useContextMenu, useDropdownMenu } from '@sd/ui';
import { useScrolled } from '~/hooks/useScrolled';
import CreateDialog from '../settings/library/tags/CreateDialog';

export default (props: { objectId: number }) => {
	const submitPlausibleEvent = usePlausibleEvent();

	const tags = useLibraryQuery(['tags.list'], { suspense: true });
	const tagsForObject = useLibraryQuery(['tags.getForObject', props.objectId], {
		suspense: true
	});

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
				keybind="⌘N"
				onClick={() => {
					dialogManager.create((dp) => (
						<CreateDialog {...dp} assignToObject={props.objectId} />
					));
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
							const active = !!tagsForObject.data?.find((t) => t.id === tag?.id);

							if (!tag) return null;
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
									onClick={(e) => {
										e.preventDefault();
										assignTag.mutate({
											tag_id: tag.id,
											object_id: props.objectId,
											unassign: active
										});
									}}
								>
									<div
										className="mr-0.5 h-[15px] w-[15px] shrink-0 rounded-full border"
										style={{
											backgroundColor:
												active && tag.color ? tag.color : 'transparent',
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
