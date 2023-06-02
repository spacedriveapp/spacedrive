import { JobReport, useLibraryMutation } from '@sd/client';
import { ProgressBar } from '@sd/ui';
import { useQueryClient } from '@tanstack/react-query';
import dayjs from 'dayjs';
import {
	Info,
	Question
} from 'phosphor-react';
import { memo } from 'react';
import { showAlertDialog } from '~/components';
import JobContainer from './JobContainer';
import useJobInfo from './useJobInfo';

interface JobProps {
	job: JobReport;
	className?: string;
	isChild?: boolean;
}

function Job({ job, className, isChild }: JobProps) {
	const queryClient = useQueryClient();

	const niceData = useJobInfo(job)[job.name] || {
		name: job.name,
		icon: Question,
		textItems: [[{ text: job.status.replace(/([A-Z])/g, ' $1').trim() }]]
	};
	const isRunning = job.status === 'Running';

	// dayjs from seconds to time
	// const timeText = isRunning ? formatEstimatedRemainingTime(job.estimated_completion) : undefined;

	const clearJob = useLibraryMutation(['jobs.clear'], {
		onError: () => {
			showAlertDialog({
				title: 'Error',
				value: 'There was an error clearing the job. Please try again.'
			});
		},
		onSuccess: () => {
			queryClient.invalidateQueries(['jobs.getHistory']);
		}
	});


	// const clearJobHandler = useCallback(
	// 	(id: string) => {
	// 		clearJob.mutate(id);
	// 	},
	// 	[clearJob]
	// );

	// I don't like sending TSX as a prop due to lack of hot-reload, but it's the only way to get the error log to show up
	if (job.status === "CompletedWithErrors") {
		const JobError = (
			<pre className='custom-scroll inspector-scroll max-h-[300px] rounded border border-app-darkBox bg-app-darkBox/80 p-3'>
				{job.errors_text.map((error, i) =>
					<p
						className='mb-1 w-full overflow-auto whitespace-normal break-words text-sm'
						key={i}>
						{error.trim()}
					</p>
				)}
			</pre>
		);
		niceData.textItems?.push([{
			text: "Completed with errors", icon: Info, onClick: () => {
				showAlertDialog({
					title: 'Error',
					description: 'The job completed with errors. Please see the error log below for more information. If you need help, please contact support and provide this error.',
					children: JobError

				});
			}
		}])
	}

	return (
		<JobContainer
			className={className}
			name={niceData.name}
			circleIcon={niceData.icon}
			textItems={job.status === 'Queued' ? [[{ text: "Queued" }]] : niceData.textItems}
			isChild={job.action !== null}
		>
			{isRunning && (
				<div className="my-1 w-[335px]">
					<ProgressBar value={job.completed_task_count} total={job.task_count} />
				</div>
			)}
		</JobContainer>
	)
}


export default memo(Job);

function formatEstimatedRemainingTime(end_date: string) {
	const duration = dayjs.duration(new Date(end_date).getTime() - Date.now());

	if (duration.hours() > 0) {
		return `${duration.hours()} hour${duration.hours() > 1 ? 's' : ''} remaining`;
	} else if (duration.minutes() > 0) {
		return `${duration.minutes()} minute${duration.minutes() > 1 ? 's' : ''} remaining`;
	} else {
		return `${duration.seconds()} second${duration.seconds() > 1 ? 's' : ''} remaining`;
	}
}
