import { useBridgeQuery } from '@sd/client';
import { Input } from '@sd/ui';
import { Database } from 'phosphor-react';
import React from 'react';

import Card from '../../../components/layout/Card';
import { Toggle } from '../../../components/primitive';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function GeneralSettings() {
	const { data: node } = useBridgeQuery(['getNode']);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="General Settings"
				description="General settings related to this client."
			/>
			<Card className="px-5 dark:bg-gray-600">
				<div className="flex flex-col w-full my-2">
					<div className="flex">
						<span className="font-semibold">Connected Node</span>
						<div className="flex-grow" />
						<div className="space-x-2">
							<span className="px-2 py-[2px] rounded text-xs font-medium bg-gray-500">0 Peers</span>
							<span className="px-1.5 py-[2px] rounded text-xs font-medium bg-primary-600">
								Running
							</span>
						</div>
					</div>

					<hr className="mt-2 mb-4 border-gray-500 " />
					<div className="flex flex-row space-x-4">
						<div className="flex flex-col">
							<span className="mb-1 text-xs font-medium text-gray-700 dark:text-gray-100">
								Node Name
							</span>
							<Input value={node?.name} />
						</div>
						<div className="flex flex-col w-[100px]">
							<span className="mb-1 text-xs font-medium text-gray-700 dark:text-gray-100">
								Node Port
							</span>
							<Input contentEditable={false} value={node?.p2p_port || 5795} />
						</div>
						<div className="flex flex-col w-[295px]">
							<span className="mb-1 text-xs font-medium text-gray-700 dark:text-gray-100">
								Node ID
							</span>
							<Input contentEditable={false} value={node?.id} />
						</div>
					</div>
					<div className="flex items-center mt-5 space-x-3">
						<Toggle size="sm" value />
						<span className="text-sm text-gray-200">Run daemon when app closed</span>
					</div>
					<div className="mt-3">
						<span className="text-xs font-medium text-gray-700 dark:text-gray-400">
							<Database className="inline w-4 h-4 mr-2 -mt-[2px]" />
							<b className="mr-2">Data Folder</b>
							{node?.data_path}
						</span>
					</div>
				</div>
			</Card>
		</SettingsContainer>
	);
}
