import { useBridgeCommand, useBridgeQuery } from '@sd/client';
import { Button } from '@sd/ui';
import React, { useContext } from 'react';

import { AppPropsContext } from '../AppPropsContext';
import CodeBlock from '../components/primitive/Codeblock';

export const DebugScreen: React.FC<{}> = (props) => {
	const appPropsContext = useContext(AppPropsContext);
	const { data: client } = useBridgeQuery('NodeGetState');
	const { data: jobs } = useBridgeQuery('JobGetRunning');
	const { data: jobHistory } = useBridgeQuery('JobGetHistory');
	// const { mutate: purgeDB } = useBridgeCommand('PurgeDatabase', {
	//   onMutate: () => {
	//     alert('Database purged');
	//   }
	// });
	const { mutate: identifyFiles } = useBridgeCommand('IdentifyUniqueFiles');
	return (
		<div className="flex flex-col w-full h-screen p-5 custom-scroll page-scroll">
			<div className="flex flex-col space-y-5 pb-7">
				<h1 className="text-lg font-bold ">Developer Debugger</h1>
				<div className="flex flex-row pb-4 space-x-2">
					<Button
						className="w-40"
						variant="gray"
						size="sm"
						onClick={() => {
							if (client && appPropsContext?.onOpen) {
								appPropsContext.onOpen(client.data_path);
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
				<h1 className="text-sm font-bold ">Client State</h1>
				<CodeBlock src={{ ...client }} />
			</div>
		</div>
	);
};
