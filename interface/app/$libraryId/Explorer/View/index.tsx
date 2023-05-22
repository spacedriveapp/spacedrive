import clsx from 'clsx';
import { HTMLAttributes, PropsWithChildren, memo, useRef } from 'react';
import { createSearchParams, useMatch, useNavigate } from 'react-router-dom';
import { ExplorerItem, isPath, useLibraryContext, useLibraryMutation } from '@sd/client';
import { useExplorerConfigStore } from '~/hooks';
import { ExplorerLayoutMode, getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { usePlatform } from '~/util/Platform';
import { TOP_BAR_HEIGHT } from '../../TopBar';
import DismissibleNotice from '../DismissibleNotice';
import ContextMenu from '../File/ContextMenu';
import { ViewContext } from '../ViewContext';
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
		} else if (openFilePath && filePath && explorerConfig.openOnDoubleClick) {
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

interface Props {
	items: ExplorerItem[] | null;
	layout: ExplorerLayoutMode;
	scrollRef: React.RefObject<HTMLDivElement>;
	onLoadMore?(): void;
	hasNextPage?: boolean;
	isFetchingNextPage?: boolean;
	selectedItems?: number[];
	onSelectedChange?(selectedItems: number[]): void;
	overscan?: number;
}

export default memo((props: Props) => {
	return (
		<ViewContext.Provider
			value={{
				data: props.items,
				scrollRef: props.scrollRef,
				onLoadMore: props.onLoadMore,
				hasNextPage: props.hasNextPage,
				isFetchingNextPage: props.isFetchingNextPage,
				selectedItems: new Set(props.selectedItems),
				onSelectedChange: (selected) => props.onSelectedChange?.([...selected]),
				overscan: props.overscan
			}}
		>
			{props.layout === 'grid' && <GridView />}
			{props.layout === 'rows' && <ListView />}
			{props.layout === 'media' && <MediaView />}
		</ViewContext.Provider>
	);
});
