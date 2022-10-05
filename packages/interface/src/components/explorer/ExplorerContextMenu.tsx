import {
	getExplorerStore,
	useExplorerStore,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/client';
import { ContextMenu as CM } from '@sd/ui';
import {
	ArrowBendUpRight,
	FilePlus,
	FileX,
	LockSimple,
	Package,
	Plus,
	Share,
	TagSimple,
	Trash,
	TrashSimple
} from 'phosphor-react';
import { useSnapshot } from 'valtio';

const AssignTagMenuItems = (props: { objectId: number }) => {
	const tags = useLibraryQuery(['tags.list'], { suspense: true });
	const tagsForFile = useLibraryQuery(['tags.getForFile', props.objectId], { suspense: true });

	const { mutate: assignTag } = useLibraryMutation('tags.assign');

	return (
		<>
			{tags.data?.map((tag) => {
				const active = !!tagsForFile.data?.find((t) => t.id === tag.id);

				return (
					<CM.Item
						key={tag.id}
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

interface Props {
	children: React.ReactNode;
}

export default function ExplorerContextMenu(props: Props) {
	const store = getExplorerStore();

	return (
		<div className="relative">
			<CM.ContextMenu trigger={props.children}>
				<CM.Item label="Open" />
				<CM.Item label="Open with..." />

				<CM.Separator />

				<CM.Item label="Quick view" />
				<CM.Item label="Open in Finder" />

				<CM.Separator />

				<CM.Item label="Rename" />
				<CM.Item label="Duplicate" />

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
					<CM.Item label="Encrypt" icon={LockSimple} />
					<CM.Item label="Compress" icon={Package} />
					<CM.SubMenu label="Convert to" icon={ArrowBendUpRight}>
						<CM.Item label="PNG" />
						<CM.Item label="WebP" />
					</CM.SubMenu>
					<CM.Item variant="danger" label="Secure delete" icon={TrashSimple} />
				</CM.SubMenu>

				<CM.Separator />

				<CM.Item icon={Trash} label="Delete" variant="danger" />
			</CM.ContextMenu>
		</div>
	);
}
