import { Pencil, Plus, Trash } from '@phosphor-icons/react';
import { PropsWithChildren } from 'react';
import { useNavigate } from 'react-router';
import { Link } from 'react-router-dom';
import { ContextMenu as CM, dialogManager } from '@sd/ui';
import CreateDialog from '~/app/$libraryId/settings/library/tags/CreateDialog';
import DeleteDialog from '~/app/$libraryId/settings/library/tags/DeleteDialog';
import { useLocale } from '~/hooks';

import { useSidebarContext } from '../../SidebarLayout/Context';

export const ContextMenu = ({ children, tagId }: PropsWithChildren<{ tagId: number }>) => {
	const navigate = useNavigate();

	const sidebar = useSidebarContext();

	const { t } = useLocale();

	return (
		<CM.Root
			trigger={children}
			onOpenChange={(open) => sidebar.onLockedChange(open)}
			className="z-[100]"
		>
			<CM.Item
				icon={Plus}
				label={t('new_tag')}
				onClick={() => {
					dialogManager.create((dp) => <CreateDialog {...dp} />);
				}}
			/>
			<CM.Item
				icon={Pencil}
				label={t('edit')}
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
					label={t('delete')}
					variant="danger"
				/>
			</Link>
		</CM.Root>
	);
};
