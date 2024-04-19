import { useCallback, useEffect } from 'react';
import { useExplorerContext } from '~/app/$libraryId/Explorer/Context';
import { LIST_VIEW_ICON_SIZES } from '~/app/$libraryId/Explorer/View/ListView/useTable';

import { useOperatingSystem } from './useOperatingSystem';

/**
 * Hook that allows resizing of items in the Explorer view using the mouse wheel.
 */

export const useMouseItemResize = () => {
	const os = useOperatingSystem();
	const explorer = useExplorerContext();
	const { layoutMode } = explorer.useSettingsSnapshot();

	const handleWheel = useCallback(
		(e: WheelEvent) => {
			const isList = layoutMode === 'list';
			const deltaYModifier = isList ? Math.sign(e.deltaY) : e.deltaY / 10; // Sensitivity adjustment
			const newSize =
				Number(
					isList
						? explorer.settingsStore.listViewIconSize
						: explorer.settingsStore.gridItemSize
				) + deltaYModifier;

			const minSize = isList ? 0 : 60;
			const maxSize = isList ? 2 : 200;
			const clampedSize = Math.max(minSize, Math.min(maxSize, newSize));

			if (isList) {
				explorer.settingsStore.listViewIconSize = String(
					clampedSize
				) as keyof typeof LIST_VIEW_ICON_SIZES;
			} else if (layoutMode === 'grid') {
				explorer.settingsStore.gridItemSize = Number(clampedSize.toFixed(0));
			}
		},
		[explorer.settingsStore, layoutMode]
	);

	useEffect(() => {
		if (os !== 'windows') return;

		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === 'Control') {
				document.addEventListener('wheel', handleWheel);
			}
		};

		const handleKeyUp = (e: KeyboardEvent) => {
			if (e.key === 'Control') {
				document.removeEventListener('wheel', handleWheel);
			}
		};

		document.addEventListener('keydown', handleKeyDown);
		document.addEventListener('keyup', handleKeyUp);

		return () => {
			document.removeEventListener('keydown', handleKeyDown);
			document.removeEventListener('keyup', handleKeyUp);
		};
	}, [os, handleWheel]);
};
