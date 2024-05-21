import { Pencil, Trash } from '@phosphor-icons/react';
import { LibraryConfigWrapped } from '@sd/client';
import { Button, ButtonLink, Card, dialogManager, Tooltip } from '@sd/ui';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';

import DeleteDialog from './DeleteDialog';

interface Props {
	library: LibraryConfigWrapped;
	current: boolean;
}

export default (props: Props) => {
	const { t } = useLocale();

	return (
		<Card className="items-center">
			{/* <DotsSixVertical weight="bold" className="mt-[15px] mr-3 opacity-30" /> */}
			<Icon name="Database" alt="Database icon" size={30} className="mr-3" />
			<div className="my-0.5 flex-1">
				<h3 className="font-semibold">
					{props.library.config.name}
					{props.current && (
						<span className="ml-2 rounded bg-accent px-1.5 py-[2px] text-xs font-medium text-white">
							{t('current')}
						</span>
					)}
				</h3>
				<p className="mt-0.5 text-xs text-ink-dull">{props.library.uuid}</p>
			</div>
			<div className="flex flex-row items-center space-x-2">
				{/* <Button className="!p-1.5" variant="gray">
				<Tooltip label="TODO">
					<Database className="h-4 w-4" />
				</Tooltip>
			</Button> */}
				<ButtonLink
					className="!p-1.5"
					to={`/${props.library.uuid}/settings/library/general`}
					variant="gray"
				>
					<Tooltip label={t('edit_library')}>
						<Pencil className="size-4" />
					</Tooltip>
				</ButtonLink>
				<Button
					className="!p-1.5"
					variant="gray"
					onClick={() => {
						dialogManager.create((dp) => (
							<DeleteDialog {...dp} libraryUuid={props.library.uuid} />
						));
					}}
				>
					<Tooltip label={t('delete_library')}>
						<Trash className="size-4" />
					</Tooltip>
				</Button>
			</div>
		</Card>
	);
};
