import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { ContextMenu as CM } from '@sd/ui';
import {
	ArrowBendUpRight,
	Image,
	LockSimple,
	Package,
	Plus,
	Repeat,
	Share,
	Shield,
	ShieldCheck,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import { PropsWithChildren, useEffect, useMemo } from 'react';

import { useOperatingSystem } from '../../hooks/useOperatingSystem';
import { usePlatform } from '../../util/Platform';
import { getExplorerStore } from '../../util/explorerStore';
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

export default function ExplorerContextMenu(props: PropsWithChildren) {
	const store = getExplorerStore();
	const platform = usePlatform();
	const os = useOperatingSystem();

	const generateThumbsForLocation = useLibraryMutation('jobs.generateThumbsForLocation');
	const objectValidator = useLibraryMutation('jobs.objectValidator');
	const rescanLocation = useLibraryMutation('locations.fullRescan');

	const osFileBrowserName = useMemo(() => {
		if (os === 'macOS') {
			return 'Finder';
		} else {
			return 'Explorer';
		}
	}, [os]);

	useEffect(() => {
		return () => {
			getExplorerStore().contextMenuActiveItem = null;
		};
	}, []);

	const objectData = store.contextMenuActiveItem
		? isObject(store.contextMenuActiveItem)
			? store.contextMenuActiveItem
			: store.contextMenuActiveItem.object
		: null;

	const hasItem = store.contextMenuActiveItem !== null;

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
							console.log('TODO', store.contextMenuActiveItem);
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

				{hasItem && (
					<CM.SubMenu label="Assign tag" icon={TagSimple}>
						<AssignTagMenuItems objectId={objectData?.id || 0} />
					</CM.SubMenu>
				)}

				<CM.SubMenu label="More actions..." icon={Plus}>
					{hasItem && (
						<>
							<CM.Item label="Encrypt" icon={LockSimple} keybind="⌘E" />
							<CM.Item label="Compress" icon={Package} keybind="⌘B" />

							<CM.SubMenu label="Convert to" icon={ArrowBendUpRight}>
								<CM.Item label="PNG" />
								<CM.Item label="WebP" />
							</CM.SubMenu>
						</>
					)}

					<CM.Item
						onClick={() => store.locationId && rescanLocation.mutate(store.locationId)}
						label="Re-index"
						icon={Repeat}
					/>
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

					{hasItem && <CM.Item variant="danger" label="Secure delete" icon={TrashSimple} />}
				</CM.SubMenu>

				<CM.Separator />

				{hasItem && <CM.Item icon={Trash} label="Delete" variant="danger" keybind="⌘DEL" />}
			</CM.ContextMenu>
		</div>
	);
}
