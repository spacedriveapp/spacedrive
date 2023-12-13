import { Pencil, Plus, Trash } from '@phosphor-icons/react';
import { PropsWithChildren } from 'react';
import { useNavigate } from 'react-router';
import { Link } from 'react-router-dom';
import { ContextMenu as CM, dialogManager } from '@sd/ui';
import CreateDialog from '~/app/$libraryId/settings/library/tags/CreateDialog';
import DeleteDialog from '~/app/$libraryId/settings/library/tags/DeleteDialog';

export const ContextMenu = ({ children, tagId }: PropsWithChildren<{ tagId: number }>) => {
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
			<Link to={`settings/library/tags/${tagId}`}>
				<CM.Item
					onClick={() => {
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
			</Link>
		</CM.Root>
	);
};
