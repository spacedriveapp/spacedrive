import { JobReport, useLibraryMutation } from '@sd/client';
import { ProgressBar } from '@sd/ui';
import dayjs from 'dayjs';
import {
	Info,
	Question
} from 'phosphor-react';
import { memo, useCallback } from 'react';
import JobContainer from './JobContainer';
import useJobInfo from './useJobInfo';
import { showAlertDialog } from '~/components';
import { useQueryClient } from '@tanstack/react-query';

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

	if (job.status === "CompletedWithErrors") {
		niceData.textItems?.push([{
			text: "Completed with errors", icon: Info, onClick: () => {
				showAlertDialog({
					title: 'Error',
					description: 'The job completed with errors. Please check the error log for more information.',
					// map error text to string with newlines per error
					value: "",
					children: <pre className=' p-3 rounded border bg-app-darkBox/80 border-app-darkBox'>{job.errors_text.map((error, i) => <p className='text-sm' key={i}>{error.trim()}</p>)}</pre>
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
