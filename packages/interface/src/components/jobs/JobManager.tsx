import { EyeIcon, FolderIcon, PhotoIcon, XMarkIcon } from '@heroicons/react/24/outline';
import { useLibraryQuery } from '@sd/client';
import { JobReport } from '@sd/client';
import { Button } from '@sd/ui';
import clsx from 'clsx';
import dayjs from 'dayjs';
import { ArrowsClockwise } from 'phosphor-react';

import { Tooltip } from '../tooltip/Tooltip';

interface JobNiceData {
	name: string;
	icon: React.FC<React.ComponentProps<'svg'>>;
}

const NiceData: Record<string, JobNiceData> = {
	indexer: {
		name: 'Indexed location',
		icon: FolderIcon
	},
	thumbnailer: {
		name: 'Generated thumbnails',
		icon: PhotoIcon
	},
	file_identifier: {
		name: 'Identified unique files',
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
			{/* <div className="z-10 flex flex-row w-full h-10 bg-gray-500 border-b border-gray-700 bg-opacity-30"></div> */}
			<div className="h-full mr-1 overflow-x-hidden custom-scroll inspector-scroll">
				<div className="py-1 pl-2">
					<div className="fixed flex items-center h-10">
						<h3 className="mt-1.5 ml-2 text-md font-medium opacity-40">Recent Jobs</h3>
					</div>
					<div className="h-10"></div>
					{jobs.data?.map((job) => {
						const color = StatusColors[job.status];
						const niceData = NiceData[job.name];

						return (
							<div
								className="flex items-center px-2 py-2 border-b border-gray-500 bg-opacity-60"
								key={job.id}
							>
								<Tooltip label={job.status}>
									<niceData.icon className={clsx('w-5 mr-3', color)} />
								</Tooltip>
								<div className="flex flex-1 flex-col">
									<span className="mt-0.5 font-semibold">{niceData.name}</span>
									<div className="flex items-center opacity-60">
										<span className="text-xs">
											{job.status === 'Failed' ? 'Failed after' : 'Took'}{' '}
											{job.seconds_elapsed
												? dayjs.duration({ seconds: job.seconds_elapsed }).humanize()
												: 'less than a second'}
										</span>
										<span className="mx-1 opacity-50">&#8226;</span>
										<span className="text-xs">{dayjs(job.date_created).toNow(true)} ago</span>
									</div>
									<span className="text-xs">{job.data}</span>
								</div>
								<div className="space-x-2">
									{job.status === 'Failed' && (
										<Button padding="thin" variant="gray">
											<ArrowsClockwise className="w-4 h-4" />
										</Button>
									)}
									<Button padding="thin" variant="gray">
										<XMarkIcon className="w-4 h-4" />
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
