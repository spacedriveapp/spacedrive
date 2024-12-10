import { HTMLAttributes, ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { useSelector, type ExplorerItem } from '@sd/client';
import { useOperatingSystem } from '~/hooks';
import { useRoutingContext } from '~/RoutingContext';

import { useExplorerContext } from '../../Context';
import { explorerStore, isCut } from '../../store';

interface Props extends Omit<HTMLAttributes<HTMLDivElement>, 'children'> {
	item: ExplorerItem;
	children: (state: { selected: boolean; cut: boolean }) => ReactNode;
}

export const SimpleGridItem = ({ children, item, ...props }: Props) => {
	const explorer = useExplorerContext();
	const { currentIndex, maxIndex } = useRoutingContext();
	const os = useOperatingSystem();
	const navigate = useNavigate();

	const cutCopyState = useSelector(explorerStore, (s) => s.cutCopyState);
	const cut = isCut(item, cutCopyState);
	const selected = explorer.selectedItems.has(item);

	const canGoBack = currentIndex !== 0;
	const canGoForward = currentIndex !== maxIndex;

	return (
		<div
			{...props}
			className="size-full"
			onMouseDown={(e) => {
				e.stopPropagation();
				if (os === 'browser') return;
				if (e.buttons === 8 || e.buttons === 3) {
					if (!canGoBack) return;
					navigate(-1);
				} else if (e.buttons === 16 || e.buttons === 4) {
					if (!canGoForward) return;
					navigate(1);
				}
			}}
		>
			{children({ selected, cut })}
		</div>
	);
};
