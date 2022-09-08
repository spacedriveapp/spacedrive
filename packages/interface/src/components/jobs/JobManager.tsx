import { CheckBadgeIcon, KeyIcon, RssIcon } from '@heroicons/react/24/outline';
import { EyeIcon, FolderIcon, PhotoIcon, XMarkIcon } from '@heroicons/react/24/outline';
import { useLibraryQuery } from '@sd/client';
import { JobReport } from '@sd/core';
import { Button } from '@sd/ui';
import clsx from 'clsx';
import { format, formatDistance, formatDistanceToNow, formatDuration } from 'date-fns';
import moment from 'moment';
import { ArrowsClockwise } from 'phosphor-react';
import React from 'react';

import { Tooltip } from '../tooltip/Tooltip';

interface JobNiceData {
	name: string;
	icon: React.FC<React.ComponentProps<'svg'>>;
}

const NiceData: Record<string, JobNiceData> = {
	indexer: {
		name: 'Location Indexer',
		icon: FolderIcon
	},
	thumbnailer: {
		name: 'Thumbnail Generator',
		icon: PhotoIcon
	},
	file_identifier: {
		name: 'File Identifier',
		icon: EyeIcon
	}
};

const StatusColors: Record<JobReport['status'], string> = {
	Running: 'text-blue-500',
	Failed: 'text-red-500',
	Completed: 'text-green-500',
	Queued: 'text-yellow-500',
	Canceled: 'text-gray-500',
	Paused: 'text-gray-500'
};

function elapsed(seconds: number) {
	return new Date(seconds * 1000).toUTCString().match(/(\d\d:\d\d:\d\d)/)?.[0];
}

export function JobsManager() {
	const jobs = useLibraryQuery(['jobs.getHistory']);
	return (
		<div className="h-full">
			<div className="flex flex-row w-full h-10 bg-gray-500 border-b border-gray-700 bg-opacity-30"></div>
			<div className="h-full mr-1 -mt-10 overflow-x-hidden custom-scroll inspector-scroll">
				<div className="px-2 py-4">
					{jobs.data?.map((job) => {
						const color = StatusColors[job.status];
						const niceData = NiceData[job.name];
						// if (job.metadata) console.log({ job });

						return (
							<div
								className="flex items-center px-2 py-2 border-b border-gray-700 rounded bg-opacity-60"
								key={job.id}
							>
								<Tooltip label={job.status}>
									<niceData.icon className={clsx('w-5 mr-3', color)} />
								</Tooltip>
								<div className="flex flex-col">
									<span className="flex mt-0.5 items-center font-semibold">{niceData.name}</span>
									<div className="flex items-center">
										<span className="text-xs opacity-60">
											{job.status === 'Failed' ? 'Failed after' : 'Took'}{' '}
											{job.seconds_elapsed
												? formatDuration({ seconds: job.seconds_elapsed })
												: 'less than a second'}
										</span>
										<span className="mx-1 opacity-30">&#8226;</span>
										<span className="text-xs opacity-60">
											{formatDistanceToNow(new Date(job.date_created))} ago
										</span>
									</div>
									<span className="text-xs opacity-60">{job.data}</span>
								</div>
								<div className="flex-grow" />
								<div className="flex space-x-2">
									<Button className="!p-0 w-7 h-7 flex items-center" variant="gray">
										<ArrowsClockwise className="w-4" />
									</Button>
									<Button className="!p-0 w-7 h-7 flex items-center" variant="gray">
										<XMarkIcon className="w-4" />
									</Button>
								</div>
							</div>
						);
					})}
				</div>
			</div>
		</div>
	);
}
