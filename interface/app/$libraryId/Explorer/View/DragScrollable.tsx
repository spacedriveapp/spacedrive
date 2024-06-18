import { useExplorerLayoutStore } from '@sd/client';
import { tw } from '@sd/ui';

import { useTopBarContext } from '../../TopBar/Context';
import { useExplorerContext } from '../Context';
import { PATH_BAR_HEIGHT } from '../ExplorerPathBar';
import { useDragScrollable } from './useDragScrollable';

const Trigger = tw.div`absolute inset-x-0 h-10 pointer-events-none`;

export const DragScrollable = () => {
	const topBar = useTopBarContext();
	const explorer = useExplorerContext();
	const explorerSettings = explorer.useSettingsSnapshot();

	const layoutStore = useExplorerLayoutStore();
	const showPathBar = explorer.showPathBar && layoutStore.showPathBar;

	const { ref: dragScrollableUpRef } = useDragScrollable({ direction: 'up' });
	const { ref: dragScrollableDownRef } = useDragScrollable({ direction: 'down' });

	return (
		<>
			{explorerSettings.layoutMode !== 'list' && (
				<Trigger ref={dragScrollableUpRef} style={{ top: topBar.topBarHeight }} />
			)}
			<Trigger
				ref={dragScrollableDownRef}
				style={{ bottom: showPathBar ? PATH_BAR_HEIGHT : 0 }}
			/>
		</>
	);
};
