import { ExplorerItem } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { ExtraFn, SharedItems } from '..';

interface Props {
	data: Extract<ExplorerItem, { type: 'Location' }>;
	extra?: ExtraFn;
}

export default ({ data, extra }: Props) => {
	const location = data.item;

	return (
		<>
			<SharedItems.OpenQuickView item={data} />

			<ContextMenu.Separator />

			<SharedItems.Details />

			<ContextMenu.Separator />

			<SharedItems.RevealInNativeExplorer locationId={location.id} />

			{extra?.({ location })}

			<ContextMenu.Separator />

			<SharedItems.Share />
		</>
	);
};
