import { HTMLAttributes, PropsWithChildren, ReactNode, memo } from 'react';
import { createSearchParams, useNavigate } from 'react-router-dom';
import { ExplorerItem, isPath, useLibraryContext, useLibraryMutation } from '@sd/client';
import { useExplorerConfigStore } from '~/hooks';
import { ExplorerLayoutMode, getExplorerStore } from '~/hooks/useExplorerStore';
import { usePlatform } from '~/util/Platform';
import ContextMenu from '../File/ContextMenu';
import { ExplorerViewContext, ExplorerViewSelection, ViewContext } from '../ViewContext';
import { getExplorerItemData, getItemFilePath } from '../util';
import GridView from './GridView';
import ListView from './ListView';
import MediaView from './MediaView';

interface ViewItemProps extends PropsWithChildren, HTMLAttributes<HTMLDivElement> {
	data: ExplorerItem;
	index: number;
	contextMenuClassName?: string;
}

export const ViewItem = ({
	data,
	index,
	children,
	contextMenuClassName,
	...props
}: ViewItemProps) => {
	const explorerStore = useExplorerStore();
	const { library } = useLibraryContext();
	const navigate = useNavigate();

	const { openFilePath } = usePlatform();
	const updateAccessTime = useLibraryMutation('files.updateAccessTime');
	const filePath = getItemFilePath(data);

	const explorerConfig = useExplorerConfigStore();

	const onDoubleClick = () => {
		if (isPath(data) && data.item.is_dir) {
			navigate({
				pathname: `/${library.uuid}/location/${getItemFilePath(data)?.location_id}`,
				search: createSearchParams({
					path: `${data.item.materialized_path}${data.item.name}/`
				}).toString()
			});

			getExplorerStore().selectedRowIndex = null;
		} else if (
			openFilePath &&
			filePath &&
			explorerConfig.openOnDoubleClick &&
			!explorerStore.isRenaming
		) {
			data.type === 'Path' &&
				data.item.object_id &&
				updateAccessTime.mutate(data.item.object_id);
			openFilePath(library.uuid, filePath.id);
		} else {
			const { kind } = getExplorerItemData(data);

			if (['Video', 'Image', 'Audio'].includes(kind)) {
				getExplorerStore().quickViewObject = data;
			}
		}
	};

	const onClick = (e: React.MouseEvent<HTMLDivElement>) => {
		// e.stopPropagation();
		// getExplorerStore().selectedRowIndex = index;
	};

	return (
		<ContextMenu data={data} className={contextMenuClassName}>
			<div
				onClick={onClick}
				onDoubleClick={onDoubleClick}
				onContextMenu={() => (getExplorerStore().selectedRowIndex = index)}
				{...props}
			>
				{children}
			</div>
		</ContextMenu>
	);
};

interface Props<T extends ExplorerViewSelection> extends ExplorerViewContext<T> {
	layout: ExplorerLayoutMode;
}

export default memo(({ layout, ...contextProps }) => {
	return (
		<ViewContext.Provider value={contextProps as ExplorerViewContext}>
			{layout === 'grid' && <GridView />}
			{layout === 'rows' && <ListView />}
			{layout === 'media' && <MediaView />}
		</ViewContext.Provider>
	);
}) as <T extends ExplorerViewSelection>(props: Props<T>) => JSX.Element;
