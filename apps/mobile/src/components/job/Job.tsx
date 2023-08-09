import {
	Copy,
	Fingerprint,
	Folder,
	Icon,
	Image,
	Info,
	Scissors,
	Trash
} from 'phosphor-react-native';
import { memo, useEffect, useState } from 'react';
import { ViewStyle } from 'react-native';
import { JobProgressEvent, JobReport, useJobInfo, useLibrarySubscription } from '@sd/client';
import JobContainer from './JobContainer';

type JobProps = {
	job: JobReport;
	isChild?: boolean;
	containerStyle?: ViewStyle;
};

const JobIcon: Record<string, Icon> = {
	indexer: Folder,
	thumbnailer: Image,
	file_identifier: Fingerprint,
	file_copier: Copy,
	file_deleter: Trash,
	file_cutter: Scissors,
	object_validator: Fingerprint
};

function Job({ job, isChild }: JobProps) {
	const [realtimeUpdate, setRealtimeUpdate] = useState<JobProgressEvent | null>(null);

	useLibrarySubscription(['jobs.progress', job.id], {
		onData: setRealtimeUpdate
	});

	const jobData = useJobInfo(job, realtimeUpdate);

	// clear stale realtime state when job is done
	useEffect(() => {
		if (jobData.isRunning) setRealtimeUpdate(null);
	}, [jobData.isRunning]);

	if (job.status === 'CompletedWithErrors') {
		// TODO:
		// const JobError = (
		// 	<pre className="custom-scroll inspector-scroll max-h-[300px] rounded border border-app-darkBox bg-app-darkBox/80 p-3">
		// 		{job.errors_text.map((error, i) => (
		// 			<p
		// 				className="mb-1 w-full overflow-auto whitespace-normal break-words text-sm"
		// 				key={i}
		// 			>
		// 				{error.trim()}
		// 			</p>
		// 		))}
		// 	</pre>
		// );
		jobData.textItems?.push([
			{
				text: 'Completed with errors',
				icon: Info as any,
				onClick: () => {
					// TODO:
					// 	showAlertDialog({
					// 		title: 'Error',
					// 		description:
					// 			'The job completed with errors. Please see the error log below for more information. If you need help, please contact support and provide this error.',
					// 		children: JobError
					// 	});
					// }
				}
			}
		]);
	}

	return (
		<JobContainer
			name={jobData.name}
			icon={JobIcon[job.name]}
			textItems={
				['Queued'].includes(job.status) ? [[{ text: job.status }]] : jobData.textItems
			}
			isChild={isChild}
		>
			{(jobData.isRunning || jobData.isPaused) && (
				// TODO: Progress Bar
				<></>
			)}
		</JobContainer>
	);
}

export default memo(Job);
