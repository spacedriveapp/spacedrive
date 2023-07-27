import { ExplorerItem } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { SharedItems } from '..';

interface Props {
	data: Extract<ExplorerItem, { type: 'NonIndexedPath' }>;
}

export default ({ data }: Props) => {
	const location = data.item;

	return (
		<>
			<SharedItems.OpenQuickView item={data} />

			<ContextMenu.Separator />

			<SharedItems.Details />

			<ContextMenu.Separator />

			<SharedItems.RevealInNativeExplorer locationId={location.id} />

			<ContextMenu.Separator />

			<SharedItems.Share />
		</>
	);
};
