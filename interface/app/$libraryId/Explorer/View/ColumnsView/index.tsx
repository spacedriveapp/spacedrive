import { CSSProperties, memo, useEffect } from 'react';
import { toast } from 'sonner';
import { useSelector } from '@sd/client';
import { useLayoutStore } from '~/app/$libraryId/Layout/store';
import { useTopBarContext } from '~/app/$libraryId/TopBar/Context';

import { explorerStore } from '../../store';

export const ColumnsView = memo(() => {
	const { sidebar } = useLayoutStore();
	const topBar = useTopBarContext();
	const [bottomBarHeight] = useSelector(explorerStore, (s) => [s.bottomBarHeight]);

	const topPadding = topBar.topBarHeight;

	// useEffect(() => {
	// 	toast('Bottom bar height was updated, ' + bottomBarHeight);
	// }, [bottomBarHeight]);

	return (
		// PRIMARY/PARENT HORIZONTAL VIEW
		<div
			className="explorer-scroll flex flex-row gap-2 overflow-x-scroll"
			style={
				{
					'--scrollbar-margin-bottom': '2px',
					// TODO: figure out why this **sometimes** isn't properly reactive
					'height': `calc(100vh - ${bottomBarHeight}px)`,
					'width': `calc(100vw - ${sidebar.size}px)`
				} as CSSProperties
			}
		>
			{new Array(15).fill('directory').map((path, colIndex, columns) => (
				// INDIVIDUAL COLUMN
				<div
					className="explorer-scroll flex min-w-48 flex-col gap-0 overflow-y-scroll border-r-[1px] border-r-app-box px-3 py-2 last:border-r-0"
					key={`col-${colIndex}-parent`}
				>
					<h3 className="sr-only">
						{path} #{colIndex + 1}/{columns.length}
					</h3>
					<div
						style={{
							marginTop: topPadding
						}}
					/>
					{new Array(100).fill('file').map((path, rowIndex) => (
						<div
							className="rounded px-1.5 py-0.5"
							key={`col-${colIndex}-row-${rowIndex}`}
						>
							{path} #{rowIndex + 1}
						</div>
					))}
				</div>
			))}
		</div>
	);
});
