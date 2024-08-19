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
import { memo } from 'react';
import { View, ViewStyle } from 'react-native';
import { JobProgressEvent, Report, useJobInfo } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import { ProgressBar } from '../animation/ProgressBar';
import JobContainer from './JobContainer';

type JobProps = {
	job: Report;
	isChild?: boolean;
	containerStyle?: ViewStyle;
	progress: JobProgressEvent | null;
};

const JobIcon: Record<string, Icon> = {
	Indexer: Folder,
	MediaProcessor: Image,
	FileIdentifier: Fingerprint,
	FileCopier: Copy,
	FileDeleter: Trash,
	FileCutter: Scissors,
	ObjectValidator: Fingerprint
};

function Job({ job, isChild, progress, containerStyle }: JobProps) {
	const jobData = useJobInfo(job, progress);

	if (job.status === 'CompletedWithErrors') {
		// TODO:
		// const JobError = (
		// 	<pre className="custom-scroll inspector-scroll max-h-[300px] rounded border border-app-darkBox bg-app-darkBox/80 p-3">
		// 		{job.errors_text.map((error, i) => (
		// 			<p
		// 				className="w-full mb-1 overflow-auto text-sm break-words whitespace-normal"
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
			containerStyle={twStyle(containerStyle)}
			name={jobData.name}
			icon={JobIcon[job.name]}
			textItems={
				['Queued'].includes(job.status) ? [[{ text: job.status }]] : jobData.textItems
			}
			isChild={isChild}
		>
			{(jobData.isRunning || jobData.isPaused) && (
				<View style={tw`my-1.5 ml-1.5 w-[300px]`}>
					<ProgressBar
						pending={jobData.taskCount == 0}
						value={jobData.completedTaskCount}
						total={jobData.taskCount}
					/>
				</View>
			)}
		</JobContainer>
	);
}

export default memo(Job);
