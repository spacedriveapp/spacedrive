import { ExplorerItem, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { ContextMenu as CM } from '@sd/ui';
import { dialogManager } from '@sd/ui';
import {
	ArrowBendUpRight,
	Image,
	LockSimple,
	LockSimpleOpen,
	Package,
	Plus,
	Repeat,
	Share,
	ShieldCheck,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import { PropsWithChildren, useMemo } from 'react';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { showAlertDialog } from '~/util';
import { usePlatform } from '~/util/Platform';

import { DecryptFileDialog } from '../dialog/DecryptFileDialog';
import { DeleteFileDialog } from '../dialog/DeleteFileDialog';
import { EncryptFileDialog } from '../dialog/EncryptFileDialog';
import { EraseFileDialog } from '../dialog/EraseFileDialog';
import { isObject } from './utils';

const AssignTagMenuItems = (props: { objectId: number }) => {
	const tags = useLibraryQuery(['tags.list'], { suspense: true });
	const tagsForObject = useLibraryQuery(['tags.getForObject', props.objectId], { suspense: true });
	const { mutate: assignTag } = useLibraryMutation('tags.assign');

	return (
		<>
			{tags.data?.map((tag, index) => {
				const active = !!tagsForObject.data?.find((t) => t.id === tag.id);

				return (
					<CM.Item
						key={tag.id}
						keybind={`${index + 1}`}
						onClick={(e) => {
							e.preventDefault();
							if (props.objectId === null) return;

							assignTag({
								tag_id: tag.id,
								object_id: props.objectId,
								unassign: active
							});
						}}
					>
						<div
							className="block w-[15px] h-[15px] mr-0.5 border rounded-full"
							style={{
								backgroundColor: active ? tag.color || '#efefef' : 'transparent' || '#efefef',
								borderColor: tag.color || '#efefef'
							}}
						/>
						<p>{tag.name}</p>
					</CM.Item>
				);
			})}
		</>
	);
};

function OpenInNativeExplorer() {
	const platform = usePlatform();
	const os = useOperatingSystem();

	const osFileBrowserName = useMemo(() => {
		if (os === 'macOS') {
			return 'Finder';
		} else {
			return 'Explorer';
		}
	}, [os]);

	return (
		<>
			{platform.openPath && (
				<CM.Item
					label={`Open in ${osFileBrowserName}`}
					keybind="⌘Y"
					onClick={() => {
						alert('TODO: Open in FS');
						// console.log('TODO', store.contextMenuActiveItem);
						// platform.openPath!('/Users/oscar/Desktop'); // TODO: Work out the file path from the backend
					}}
				/>
			)}
		</>
	);
}

export function ExplorerContextMenu(props: PropsWithChildren) {
	const store = getExplorerStore();

	const generateThumbsForLocation = useLibraryMutation('jobs.generateThumbsForLocation');
	const objectValidator = useLibraryMutation('jobs.objectValidator');
	const rescanLocation = useLibraryMutation('locations.fullRescan');

	return (
		<div className="relative">
			<CM.ContextMenu trigger={props.children}>
				<OpenInNativeExplorer />

				<CM.Separator />

				<CM.Item
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

				<CM.Separator />

				<CM.Item
					onClick={() => store.locationId && rescanLocation.mutate(store.locationId)}
					label="Re-index"
					icon={Repeat}
				/>

				<CM.SubMenu label="More actions..." icon={Plus}>
					<CM.Item
						onClick={() =>
							store.locationId &&
							generateThumbsForLocation.mutate({ id: store.locationId, path: '' })
						}
						label="Regen Thumbnails"
						icon={Image}
					/>
					<CM.Item
						onClick={() =>
							store.locationId && objectValidator.mutate({ id: store.locationId, path: '' })
						}
						label="Generate Checksums"
						icon={ShieldCheck}
					/>
				</CM.SubMenu>

				<CM.Separator />
			</CM.ContextMenu>
		</div>
	);
}

export interface FileItemContextMenuProps extends PropsWithChildren {
	item: ExplorerItem;
}

export function FileItemContextMenu({ ...props }: FileItemContextMenuProps) {
	const objectData = props.item ? (isObject(props.item) ? props.item : props.item.object) : null;

	const hasMasterPasswordQuery = useLibraryQuery(['keys.hasMasterPassword']);
	const hasMasterPassword =
		hasMasterPasswordQuery.data !== undefined && hasMasterPasswordQuery.data === true
			? true
			: false;

	const mountedUuids = useLibraryQuery(['keys.listMounted']);
	const hasMountedKeys =
		mountedUuids.data !== undefined && mountedUuids.data.length > 0 ? true : false;

	return (
		<div className="relative">
			<CM.ContextMenu trigger={props.children}>
				<CM.Item label="Open" keybind="⌘O" />
				<CM.Item label="Open with..." />

				<CM.Separator />

				<CM.Item label="Quick view" keybind="␣" />
				<OpenInNativeExplorer />

				<CM.Separator />

				<CM.Item label="Rename" />
				<CM.Item label="Duplicate" keybind="⌘D" />

				<CM.Separator />

				<CM.Item
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

				<CM.Separator />

				<CM.SubMenu label="Assign tag" icon={TagSimple}>
					<AssignTagMenuItems objectId={objectData?.id || 0} />
				</CM.SubMenu>

				<CM.SubMenu label="More actions..." icon={Plus}>
					<CM.Item
						label="Encrypt"
						icon={LockSimple}
						keybind="⌘E"
						onClick={() => {
							if (hasMasterPassword && hasMountedKeys) {
								dialogManager.create((dp) => (
									<EncryptFileDialog
										{...dp}
										location_id={useExplorerStore().locationId!}
										path_id={props.item.id}
									/>
								));
							} else if (!hasMasterPassword) {
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
					<CM.Item
						label="Decrypt"
						icon={LockSimpleOpen}
						keybind="⌘D"
						onClick={() => {
							if (hasMasterPassword) {
								dialogManager.create((dp) => (
									<DecryptFileDialog
										{...dp}
										location_id={useExplorerStore().locationId!}
										path_id={props.item.id}
									/>
								));
							} else {
								showAlertDialog({
									title: 'Key manager locked',
									value: 'The key manager is currently locked. Please unlock it and try again.'
								});
							}
						}}
					/>
					<CM.Item label="Compress" icon={Package} keybind="⌘B" />
					<CM.SubMenu label="Convert to" icon={ArrowBendUpRight}>
						<CM.Item label="PNG" />
						<CM.Item label="WebP" />
					</CM.SubMenu>
					<CM.Item label="Rescan Directory" icon={Package} />
					<CM.Item label="Regen Thumbnails" icon={Package} />
					<CM.Item
						variant="danger"
						label="Secure delete"
						icon={TrashSimple}
						onClick={() => {
							dialogManager.create((dp) => (
								<EraseFileDialog
									{...dp}
									location_id={getExplorerStore().locationId!}
									path_id={props.item.id}
								/>
							));
						}}
					/>
				</CM.SubMenu>

				<CM.Separator />

				<CM.Item
					icon={Trash}
					label="Delete"
					variant="danger"
					keybind="⌘DEL"
					onClick={() => {
						dialogManager.create((dp) => (
							<DeleteFileDialog
								{...dp}
								location_id={getExplorerStore().locationId!}
								path_id={props.item.id}
							/>
						));
					}}
				/>
			</CM.ContextMenu>
		</div>
	);
}
