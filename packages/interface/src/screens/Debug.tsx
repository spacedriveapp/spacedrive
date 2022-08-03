import { AppPropsContext, useBridgeQuery, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button } from '@sd/ui';
import React, { useContext } from 'react';

import CodeBlock from '../components/primitive/Codeblock';

export const DebugScreen: React.FC<{}> = (props) => {
	const appPropsContext = useContext(AppPropsContext);
	const { data: nodeState } = useBridgeQuery(['getNode']);
	const { data: libraryState } = useBridgeQuery(['library.get']);
	const { data: jobs } = useLibraryQuery(['jobs.getRunning']);
	const { data: jobHistory } = useLibraryQuery(['jobs.getHistory']);
	// const { mutate: purgeDB } = useBridgeCommand('PurgeDatabase', {
	//   onMutate: () => {
	//     alert('Database purged');
	//   }
	// });
	const { mutate: identifyFiles } = useLibraryMutation('jobs.identifyUniqueFiles');
	return (
		<div className="flex flex-col w-full h-screen custom-scroll page-scroll">
			<div data-tauri-drag-region className="flex flex-shrink-0 w-full h-5" />
			<div className="flex flex-col p-5 pt-2 space-y-5 pb-7">
				<h1 className="text-lg font-bold ">Developer Debugger</h1>
				<div className="flex flex-row pb-4 space-x-2">
					<Button
						className="w-40"
						variant="gray"
						size="sm"
						onClick={() => {
							if (nodeState && appPropsContext?.onOpen) {
								appPropsContext.onOpen(nodeState.data_path);
							}
						}}
					>
						Open data folder
					</Button>
				</div>
				<h1 className="text-sm font-bold ">Running Jobs</h1>
				<CodeBlock src={{ ...jobs }} />
				<h1 className="text-sm font-bold ">Job History</h1>
				<CodeBlock src={{ ...jobHistory }} />
				<h1 className="text-sm font-bold ">Node State</h1>
				<CodeBlock src={{ ...nodeState }} />
				<h1 className="text-sm font-bold ">Libraries</h1>
				<CodeBlock src={{ ...libraryState }} />
			</div>
		</div>
	);
};
