import { ExplorerItem } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { type ExtraFn, SharedItems } from '..';

interface Props {
	data: Extract<ExplorerItem, { type: 'NonIndexedPath' }>;
	extra?: ExtraFn;
}

export default ({ data, extra }: Props) => {
	const location = data.item;

	return (
		<>
			<SharedItems.OpenQuickView item={data} />

			<ContextMenu.Separator />

			<SharedItems.Details />

			{/* TODO: Implement reveal in native explorer for ephemeral path */}
			{/* <ContextMenu.Separator /> */}
			{/* <SharedItems.RevealInNativeExplorer locationId={location.id} /> */}

			{extra?.({ location })}

			<ContextMenu.Separator />

			<SharedItems.Share />
		</>
	);
};
