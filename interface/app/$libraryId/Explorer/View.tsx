import clsx from 'clsx';
import { HTMLAttributes, PropsWithChildren, memo, useRef } from 'react';
import { createSearchParams, useMatch, useNavigate } from 'react-router-dom';
import { ExplorerItem, isPath, useLibraryContext, useLibraryMutation } from '@sd/client';
import { getExplorerStore, useExplorerConfigStore, useExplorerStore } from '~/hooks';
import { usePlatform } from '~/util/Platform';
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
	listViewHeadersClassName?: string;
	scrollRef?: React.RefObject<HTMLDivElement>;
}

export default memo((props: Props) => {
	const explorerStore = useExplorerStore();
	const layoutMode = explorerStore.layoutMode;

	const scrollRef = useRef<HTMLDivElement>(null);

	// Hide notice on overview page
	const isOverview = useMatch('/:libraryId/overview');

	return (
		<div
			ref={props.scrollRef || scrollRef}
			className={clsx(
				'custom-scroll explorer-scroll h-screen',
				layoutMode === 'grid' && 'overflow-x-hidden',
				props.viewClassName
			)}
			style={{ paddingTop: TOP_BAR_HEIGHT }}
			onClick={() => (getExplorerStore().selectedRowIndex = null)}
		>
			{!isOverview && <DismissibleNotice />}
			<ViewContext.Provider
				value={{
					data: props.data,
					scrollRef: props.scrollRef || scrollRef,
					onLoadMore: props.onLoadMore,
					hasNextPage: props.hasNextPage,
					isFetchingNextPage: props.isFetchingNextPage
				}}
			>
				{layoutMode === 'grid' && <GridView />}
				{layoutMode === 'rows' && (
					<ListView listViewHeadersClassName={props.listViewHeadersClassName} />
				)}
				{layoutMode === 'media' && <MediaView />}
			</ViewContext.Provider>
		</div>
	);
});
