import clsx from 'clsx';
import { HTMLAttributes, PropsWithChildren, memo, useRef } from 'react';
import { createSearchParams, useMatch, useNavigate } from 'react-router-dom';
import { ExplorerItem, isPath, useLibraryContext } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { TOP_BAR_HEIGHT } from '../TopBar';
import DismissibleNotice from './DismissibleNotice';
import ContextMenu from './File/ContextMenu';
import GridView from './GridView';
import ListView from './ListView';
import MediaView from './MediaView';
import { ViewContext } from './ViewContext';
import { getExplorerItemData, getItemFilePath } from './util';

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

	const onDoubleClick = () => {
		if (isPath(data) && data.item.is_dir) {
			navigate({
				pathname: `/${library.uuid}/location/${getItemFilePath(data)?.location_id}`,
				search: createSearchParams({ path: data.item.materialized_path }).toString()
			});

			getExplorerStore().selectedRowIndex = null;
		} else {
			const { kind } = getExplorerItemData(data);

			if (['Video', 'Image', 'Audio'].includes(kind)) {
				getExplorerStore().quickViewObject = data;
			}
		}
	};

	const onClick = (e: React.MouseEvent<HTMLDivElement>) => {
		e.stopPropagation();
		getExplorerStore().selectedRowIndex = index;
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
	data: ExplorerItem[];
	onLoadMore?(): void;
	hasNextPage?: boolean;
	isFetchingNextPage?: boolean;
	viewClassName?: string;
}

export default memo((props: Props) => {
	const explorerStore = useExplorerStore();
	const layoutMode = explorerStore.layoutMode;

	const scrollRef = useRef<HTMLDivElement>(null);

	// Hide notice on overview page
	const isOverview = useMatch('/:libraryId/overview');

	return (
		<div
			ref={scrollRef}
			className={clsx(
				'custom-scroll explorer-scroll h-screen',
				layoutMode === 'grid' && 'overflow-x-hidden pl-4',
				props.viewClassName
			)}
			style={{ paddingTop: TOP_BAR_HEIGHT }}
			onClick={() => (getExplorerStore().selectedRowIndex = null)}
		>
			{!isOverview && <DismissibleNotice />}
			<ViewContext.Provider
				value={{
					data: props.data,
					scrollRef,
					onLoadMore: props.onLoadMore,
					hasNextPage: props.hasNextPage,
					isFetchingNextPage: props.isFetchingNextPage
				}}
			>
				{layoutMode === 'grid' && <GridView />}
				{layoutMode === 'rows' && <ListView />}
				{layoutMode === 'media' && <MediaView />}
			</ViewContext.Provider>
		</div>
	);
});
