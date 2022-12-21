import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { ContextMenu as CM } from '@sd/ui';
import {
	ArrowBendUpRight,
	LockSimple,
	LockSimpleOpen,
	Package,
	Plus,
	Share,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import { PropsWithChildren, useMemo } from 'react';

import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import { usePlatform } from '../../util/Platform';
import { getExplorerStore } from '../../util/explorerStore';
import { GenericAlertDialogProps } from '../dialog/AlertDialog';
import { EncryptFileDialog } from '../dialog/EncryptFileDialog';

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

export interface ExplorerContextMenuProps extends PropsWithChildren {
	setShowEncryptDialog: (isShowing: boolean) => void;
	setShowDecryptDialog: (isShowing: boolean) => void;
	setAlertDialogData: (data: GenericAlertDialogProps) => void;
}

export default function ExplorerContextMenu(props: ExplorerContextMenuProps) {
	const store = getExplorerStore();
	// const { mutate: generateThumbsForLocation } = useLibraryMutation(
	// 	'jobs.generateThumbsForLocation'
	// );
	const platform = usePlatform();
	const os = useOperatingSystem();

	const osFileBrowserName = useMemo(() => {
		if (os === 'macOS') {
			return 'Finder';
		} else {
			return 'Explorer';
		}
	}, [os]);

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
				{platform.openPath && (
					<CM.Item
						label={`Open in ${osFileBrowserName}`}
						keybind="⌘Y"
						onClick={() => {
							console.log('TODO', store.contextMenuActiveObject);
							platform.openPath!('/Users/oscar/Desktop'); // TODO: Work out the file path from the backend
						}}
					/>
				)}

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

				{store.contextMenuObjectId && (
					<CM.SubMenu label="Assign tag" icon={TagSimple}>
						<AssignTagMenuItems objectId={store.contextMenuObjectId} />
					</CM.SubMenu>
				)}
				<CM.SubMenu label="More actions..." icon={Plus}>
					<CM.Item
						label="Encrypt"
						icon={LockSimple}
						keybind="⌘E"
						onClick={() => {
							if (hasMasterPassword && hasMountedKeys) {
								props.setShowEncryptDialog(true);
							} else if (!hasMasterPassword) {
								props.setAlertDialogData({
									open: true,
									title: 'Key manager locked',
									value: 'The key manager is currently locked. Please unlock it and try again.',
									inputBox: false,
									description: ''
								});
							} else if (!hasMountedKeys) {
								props.setAlertDialogData({
									open: true,
									title: 'No mounted keys',
									description: '',
									value: 'No mounted keys were found. Please mount a key and try again.',
									inputBox: false
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
								props.setShowDecryptDialog(true);
							} else {
								props.setAlertDialogData({
									open: true,
									title: 'Key manager locked',
									value: 'The key manager is currently locked. Please unlock it and try again.',
									inputBox: false,
									description: ''
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
					<CM.Item variant="danger" label="Secure delete" icon={TrashSimple} />
				</CM.SubMenu>

				<CM.Separator />

				<CM.Item icon={Trash} label="Delete" variant="danger" keybind="⌘DEL" />
			</CM.ContextMenu>
		</div>
	);
}
