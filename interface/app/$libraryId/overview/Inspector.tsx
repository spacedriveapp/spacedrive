import { useCallback, useEffect, useRef, useState } from 'react';
import { useSnapshot } from 'valtio';
import { useCallbackToWatchResize } from '~/hooks';

import { useExplorerContext } from '../Explorer/Context';
import { Inspector } from '../Explorer/Inspector';
import { useExplorerStore } from '../Explorer/store';
import { usePageLayoutContext } from '../PageLayout/Context';

export default () => {
	const page = usePageLayoutContext();

	const explorer = useExplorerContext();
	const explorerStore = useExplorerStore();

	const settings = useSnapshot(explorer.settingsStore);

	const ref = useRef<HTMLDivElement>(null);

	const [height, setHeight] = useState(0);

	const updateHeight = useCallback(() => {
		if (!ref.current || !page.ref.current) return;

		const { scrollTop, scrollHeight, clientHeight } = page.ref.current;

		if (scrollTop < 0 || scrollTop + clientHeight > scrollHeight) return;

		const { height } = page.ref.current.getBoundingClientRect();

		const offset = ref.current.offsetTop - scrollTop;

		setHeight(height - offset);
	}, [page.ref]);

	useEffect(() => {
		const element = page.ref.current;
		if (!element) return;

		updateHeight();

		element.addEventListener('scroll', updateHeight);
		return () => element.removeEventListener('scroll', updateHeight);
	}, [page.ref, updateHeight, explorerStore.showInspector]);

	useCallbackToWatchResize(updateHeight, [updateHeight], page.ref);

	if (!explorerStore.showInspector) return null;

	return (
		<Inspector
			ref={ref}
			showThumbnail={settings.layoutMode !== 'media'}
			className="no-scrollbar sticky top-[68px] shrink-0 overscroll-y-none p-3.5 pr-1.5"
			style={{ height }}
		/>
	);
};
