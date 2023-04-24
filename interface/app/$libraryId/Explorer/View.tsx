import clsx from 'clsx';
import {
	HTMLAttributes,
	PropsWithChildren,
	RefObject,
	createContext,
	memo,
	useContext,
	useRef
} from 'react';
import { createSearchParams, useNavigate } from 'react-router-dom';
import { ExplorerItem, isPath, useLibraryContext } from '@sd/client';
import { getExplorerStore, useExplorerStore } from '~/hooks/useExplorerStore';
import { TOP_BAR_HEIGHT } from '../TopBar';
import ContextMenu from './File/ContextMenu';
import GridView from './GridView';
import ListView from './ListView';
import MediaView from './MediaView';
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
			const filePathData = getItemFilePath(data);
			navigate({
				pathname: `/${library.uuid}/location/${filePathData?.location_id}`,
				search: createSearchParams({ path: data.item.materialized_path }).toString()
			});
			getExplorerStore().selectedRowIndex = -1;
		} else {
			const itemData = getExplorerItemData(data);
			if (
				itemData.kind === 'Video' ||
				itemData.kind === 'Image' ||
				itemData.kind === 'Audio'
			) {
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
}

interface ExplorerView {
	data: ExplorerItem[];
	scrollRef: RefObject<HTMLDivElement>;
}
const context = createContext<ExplorerView>(undefined!);
export const useExplorerView = () => useContext(context);

export default memo((props: Props) => {
	const explorerStore = useExplorerStore();
	const layoutMode = explorerStore.layoutMode;

	const scrollRef = useRef<HTMLDivElement>(null);

	return (
		<div
			ref={scrollRef}
			className={clsx(
				'custom-scroll explorer-scroll h-screen',
				layoutMode === 'grid' && 'overflow-x-hidden pl-4'
			)}
			style={{ paddingTop: TOP_BAR_HEIGHT }}
			onClick={() => (getExplorerStore().selectedRowIndex = -1)}
		>
			<context.Provider value={{ data: props.data, scrollRef }}>
				{layoutMode === 'grid' && <GridView />}
				{layoutMode === 'rows' && <ListView />}
				{layoutMode === 'media' && <MediaView />}
			</context.Provider>
		</div>
	);
});
