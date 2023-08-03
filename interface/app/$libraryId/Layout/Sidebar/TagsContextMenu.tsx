import { Pencil, Plus, Trash } from 'phosphor-react';
import { useNavigate } from 'react-router';
import { ContextMenu as CM, dialogManager } from '@sd/ui';
import CreateDialog from '~/app/$libraryId/settings/library/tags/CreateDialog';
import DeleteDialog from '~/app/$libraryId/settings/library/tags/DeleteDialog';

interface Props {
	children: React.ReactNode;
	tagId: number;
}

export default ({ children, tagId }: Props) => {
	const navigate = useNavigate();
	return (
		<CM.Root trigger={children}>
			<CM.Item
				icon={Plus}
				label="New tag"
				onClick={() => {
					dialogManager.create((dp) => <CreateDialog {...dp} />);
				}}
			/>
			<CM.Item
				icon={Pencil}
				label="Edit"
				onClick={() => navigate(`settings/library/tags/${tagId}`)}
			/>
			<CM.Separator />
			<CM.Item
				onClick={() => {
					// navigate(`settings/library/tags/${tagId}`);
					dialogManager.create((dp) => (
						<DeleteDialog
							{...dp}
							tagId={tagId}
							onSuccess={() => navigate(`settings/library/tags`)}
						/>
					));
				}}
				icon={Trash}
				label="Delete"
				variant="danger"
			/>
		</CM.Root>
	);
};
