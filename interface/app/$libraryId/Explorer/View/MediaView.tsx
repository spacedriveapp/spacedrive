import { useVirtualizer } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { ArrowsOutSimple } from 'phosphor-react';
import { memo, useEffect, useMemo, useState } from 'react';
import React from 'react';
import { useKey, useOnWindowResize } from 'rooks';
import { ExplorerItem } from '@sd/client';
import { Button } from '@sd/ui';
import GridList from '~/components/GridList';
import { useDismissibleNoticeStore } from '~/hooks/useDismissibleNoticeStore';
import {
	getExplorerStore,
	getSelectedExplorerItems,
	useExplorerStore,
	useSelectedExplorerItems
} from '~/hooks/useExplorerStore';
import { ViewItem } from '.';
import Thumb from '../File/Thumb';
import { useExplorerViewContext } from '../ViewContext';

interface MediaViewItemProps {
	data: ExplorerItem;
	index: number;
	selected: boolean;
}

const MediaViewItem = memo(({ data, index, selected }: MediaViewItemProps) => {
	const explorerStore = useExplorerStore();

	console.log('MediaViewItem', data, index, selected);

	return (
		<ViewItem
			data={data}
			index={index}
			className={clsx(
				'h-full w-full overflow-hidden border-2',
				selected ? 'border-accent' : 'border-transparent'
			)}
		>
			<div
				className={clsx(
					'group relative flex aspect-square items-center justify-center hover:bg-app-selected/20',
					selected && 'bg-app-selected/20'
				)}
			>
				<Thumb
					size={0}
					data={data}
					cover={explorerStore.mediaAspectSquare}
					className="!rounded-none"
				/>

				<Button
					variant="gray"
					size="icon"
					className="absolute right-2 top-2 hidden rounded-full shadow group-hover:block"
					onClick={() => (getExplorerStore().quickViewObject = data)}
				>
					<ArrowsOutSimple />
				</Button>
			</div>
		</ViewItem>
	);
});

export default () => {
	const explorerStore = useExplorerStore();

	const {
		data,
		scrollRef,
		onLoadMore,
		hasNextPage,
		isFetchingNextPage,
		selectedItems,
		onSelectedChange,
		overscan
	} = useExplorerViewContext();

	const fetchMore = () => {
		if (hasNextPage && !isFetchingNextPage) {
			onLoadMore?.();
		}
	};

	return (
		<GridList
			scrollRef={scrollRef}
			count={data?.length || 100}
			columns={explorerStore.mediaColumns}
			onSelect={(index) => getSelectedExplorerItems().add(data?.[index]!.item.id!)}
			onDeselect={(index) => getSelectedExplorerItems().delete(data?.[index]!.item.id!)}
			selected={selectedItems}
			onSelectedChange={onSelectedChange}
			overscan={overscan}
			onLastRow={fetchMore}
		>
			{({ index, item: Item }) => {
				if (!data) {
					return (
						<Item className="!p-px">
							<div className="h-full animate-pulse bg-app-box" />
						</Item>
					);
				}

				const item = data[index];
				if (!item) return null;

				const selected = !!selectedItems?.has(item.item.id);

				return (
					<Item selectable={true} selected={selected} index={index} id={item.item.id}>
						<MediaViewItem data={item} index={index} selected={selected} />
					</Item>
				);
			}}
		</GridList>
	);
};
