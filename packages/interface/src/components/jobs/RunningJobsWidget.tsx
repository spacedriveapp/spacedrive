import { Transition } from '@headlessui/react';
import { useLibraryQuery } from '@sd/client';
import clsx from 'clsx';
import React, { DetailedHTMLProps, HTMLAttributes } from 'react';

import ProgressBar from '../primitive/ProgressBar';

const MiddleTruncatedText = ({
	children,
	...props
}: DetailedHTMLProps<HTMLAttributes<HTMLSpanElement>, HTMLSpanElement>) => {
	const text = children?.toString() ?? '';
	const first = text.substring(0, text.length / 2);
	const last = text.substring(first.length);

	// Literally black magic
	const fontFaceScaleFactor = 1.61;
	const startWidth = fontFaceScaleFactor * 5;
	const endWidth = fontFaceScaleFactor * 4;

	return (
		<div className="whitespace-nowrap overflow-hidden w-full">
			<span
				{...props}
				style={{
					maxWidth: `calc(100% - (1em * ${endWidth}))`,
					minWidth: startWidth
				}}
				className={clsx(
					props?.className,
					'text-ellipsis inline-block align-bottom whitespace-nowrap overflow-hidden'
				)}
			>
				{first}
			</span>
			<span
				{...props}
				style={{
					maxWidth: `calc(100% - (1em * ${startWidth}))`,
					direction: 'rtl'
				}}
				className={clsx(
					props?.className,
					'inline-block align-bottom whitespace-nowrap overflow-hidden'
				)}
			>
				{last}
			</span>
		</div>
	);
};

export default function RunningJobsWidget() {
	const { data: jobs } = useLibraryQuery(['jobs.getRunning']);

	return (
		<div className="flex flex-col space-y-4">
			{jobs?.map((job) => (
				<Transition
					show={true}
					enter="transition-translate ease-in-out duration-200"
					enterFrom="translate-y-24"
					enterTo="translate-y-0"
					leave="transition-translate ease-in-out duration-200"
					leaveFrom="translate-y-0"
					leaveTo="translate-y-24"
				>
					<div key={job.id} className="flex flex-col px-2 pt-1.5 pb-2 bg-gray-700 rounded">
						{/* <span className="mb-0.5 text-tiny font-bold text-gray-400">{job.status} Job</span> */}
						<MiddleTruncatedText className="mb-1.5 text-gray-450 text-tiny">
							{job.message}
						</MiddleTruncatedText>
						<ProgressBar value={job.completed_task_count} total={job.task_count} />
					</div>
				</Transition>
			))}
		</div>
	);
}
