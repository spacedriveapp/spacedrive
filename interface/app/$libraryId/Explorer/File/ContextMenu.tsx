import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import {
	ArrowBendUpRight,
	Copy,
	FileX,
	LockSimple,
	LockSimpleOpen,
	Package,
	Plus,
	Scissors,
	Share,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import { PropsWithChildren, useRef } from 'react';
import {
	ExplorerItem,
	isObject,
	useLibraryContext,
	useLibraryMutation,
	useLibraryQuery,
	usePlausibleEvent
} from '@sd/client';
import { ContextMenu, Input, dialogManager } from '@sd/ui';
import { useExplorerParams } from '~/app/$libraryId/location/$id';
import { showAlertDialog } from '~/components/AlertDialog';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useScrolled } from '~/hooks/useScrolled';
import { usePlatform } from '~/util/Platform';
import CreateDialog from '../../settings/library/tags/CreateDialog';
import { OpenInNativeExplorer } from '../ContextMenu';
import DecryptDialog from './DecryptDialog';
import DeleteDialog from './DeleteDialog';
import EncryptDialog from './EncryptDialog';
import EraseDialog from './EraseDialog';

interface Props extends PropsWithChildren {
	data: ExplorerItem;
}

export default ({ data, ...props }: Props) => {
	const { library } = useLibraryContext();
	const store = useExplorerStore();
	const params = useExplorerParams();
	const platform = usePlatform();
	const objectData = data ? (isObject(data) ? data.item : data.item.object) : null;

	const keyManagerUnlocked = useLibraryQuery(['keys.isUnlocked']).data ?? false;
	const mountedKeys = useLibraryQuery(['keys.listMounted']);
	const hasMountedKeys = mountedKeys.data?.length ?? 0 > 0;

	const copyFiles = useLibraryMutation('files.copyFiles');

	return (
		<div className="relative">
			<ContextMenu.Root trigger={props.children}>
				<ContextMenu.Item
					label="Open"
					keybind="⌘O"
					onClick={() => {
						// TODO: Replace this with a proper UI
						window.location.href = platform.getFileUrl(
							library.uuid,
							store.locationId!,
							data.item.id
						);
					}}
					icon={Copy}
				/>
				<ContextMenu.Item label="Open with..." />

				<ContextMenu.Separator />

				{!store.showInspector && (
					<>
						<ContextMenu.Item
							label="Details"
							// icon={Sidebar}
							onClick={() => (getExplorerStore().showInspector = true)}
						/>
						<ContextMenu.Separator />
					</>
				)}

				<ContextMenu.Item label="Quick view" keybind="␣" />
				<OpenInNativeExplorer />

				<ContextMenu.Separator />

				<ContextMenu.Item label="Rename" />
				<ContextMenu.Item
					label="Duplicate"
					keybind="⌘D"
					onClick={() => {
						copyFiles.mutate({
							source_location_id: store.locationId!,
							source_path_id: data.item.id,
							target_location_id: store.locationId!,
							target_path: params.path,
							target_file_name_suffix: ' copy'
						});
					}}
				/>

				<ContextMenu.Item
					label="Cut"
					keybind="⌘X"
					onClick={() => {
						getExplorerStore().cutCopyState = {
							sourceLocationId: store.locationId!,
							sourcePathId: data.item.id,
							actionType: 'Cut',
							active: true
						};
					}}
					icon={Scissors}
				/>

				<ContextMenu.Item
					label="Copy"
					keybind="⌘C"
					onClick={() => {
						getExplorerStore().cutCopyState = {
							sourceLocationId: store.locationId!,
							sourcePathId: data.item.id,
							actionType: 'Copy',
							active: true
						};
					}}
					icon={Copy}
				/>

				<ContextMenu.Item
					label="Deselect"
					hidden={!store.cutCopyState.active}
					onClick={() => {
						getExplorerStore().cutCopyState = {
							...store.cutCopyState,
							active: false
						};
					}}
					icon={FileX}
				/>

				<ContextMenu.Separator />

				<ContextMenu.Item
					label="Share"
					icon={Share}
					onClick={(e) => {
						e.preventDefault();

						navigator.share?.({
							title: 'Spacedrive',
							text: 'Check out this cool app',
							url: 'https://spacedrive.com'
						});
					}}
				/>

				<ContextMenu.Separator />

				{objectData && (
					<ContextMenu.SubMenu label="Assign tag" icon={TagSimple}>
						<AssignTagMenuItems objectId={objectData.id} />
					</ContextMenu.SubMenu>
				)}

				<ContextMenu.SubMenu label="More actions..." icon={Plus}>
					<ContextMenu.Item
						label="Encrypt"
						icon={LockSimple}
						keybind="⌘E"
						onClick={() => {
							if (keyManagerUnlocked && hasMountedKeys) {
								dialogManager.create((dp) => (
									<EncryptDialog {...dp} location_id={store.locationId!} path_id={data.item.id} />
								));
							} else if (!keyManagerUnlocked) {
								showAlertDialog({
									title: 'Key manager locked',
									value: 'The key manager is currently locked. Please unlock it and try again.'
								});
							} else if (!hasMountedKeys) {
								showAlertDialog({
									title: 'No mounted keys',
									value: 'No mounted keys were found. Please mount a key and try again.'
								});
							}
						}}
					/>
					{/* should only be shown if the file is a valid spacedrive-encrypted file (preferably going from the magic bytes) */}
					<ContextMenu.Item
						label="Decrypt"
						icon={LockSimpleOpen}
						keybind="⌘D"
						onClick={() => {
							if (keyManagerUnlocked) {
								dialogManager.create((dp) => (
									<DecryptDialog {...dp} location_id={store.locationId!} path_id={data.item.id} />
								));
							} else {
								showAlertDialog({
									title: 'Key manager locked',
									value: 'The key manager is currently locked. Please unlock it and try again.'
								});
							}
						}}
					/>
					<ContextMenu.Item label="Compress" icon={Package} keybind="⌘B" />
					<ContextMenu.SubMenu label="Convert to" icon={ArrowBendUpRight}>
						<ContextMenu.Item label="PNG" />
						<ContextMenu.Item label="WebP" />
					</ContextMenu.SubMenu>
					<ContextMenu.Item label="Rescan Directory" icon={Package} />
					<ContextMenu.Item label="Regen Thumbnails" icon={Package} />
					<ContextMenu.Item
						variant="danger"
						label="Secure delete"
						icon={TrashSimple}
						onClick={() => {
							dialogManager.create((dp) => (
								<EraseDialog
									{...dp}
									location_id={getExplorerStore().locationId!}
									path_id={data.item.id}
								/>
							));
						}}
					/>
				</ContextMenu.SubMenu>

				<ContextMenu.Separator />

				<ContextMenu.Item
					icon={Trash}
					label="Delete"
					variant="danger"
					keybind="⌘DEL"
					onClick={() => {
						dialogManager.create((dp) => (
							<DeleteDialog
								{...dp}
								location_id={getExplorerStore().locationId!}
								path_id={data.item.id}
							/>
						));
					}}
				/>
			</ContextMenu.Root>
		</div>
	);
};

const AssignTagMenuItems = (props: { objectId: number }) => {
	const platform = usePlatform();
	const submitPlausibleEvent = usePlausibleEvent({ platformType: platform.platform });

	const tags = useLibraryQuery(['tags.list'], { suspense: true });
	const tagsForObject = useLibraryQuery(['tags.getForObject', props.objectId], { suspense: true });

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
			<ContextMenu.Item
				label="New tag"
				icon={Plus}
				iconProps={{ size: 15 }}
				keybind="⌘N"
				onClick={() => {
					dialogManager.create((dp) => <CreateDialog {...dp} assignToObject={props.objectId} />);
				}}
			/>
			<ContextMenu.Separator className={clsx('mx-0 mb-0 transition', isScrolled && 'shadow')} />
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

							return (
								tag && (
									<ContextMenu.Item
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
												backgroundColor: active && tag.color ? tag.color : 'transparent',
												borderColor: tag.color || '#efefef'
											}}
										/>
										<span className="truncate">{tag.name}</span>
									</ContextMenu.Item>
								)
							);
						})}
					</div>
				</div>
			) : (
				<div className="text-ink-faint py-1 text-center text-xs">
					{tags.data ? 'No tags' : 'Failed to load tags'}
				</div>
			)}
		</>
	);
};
