import { Copy, Fingerprint, Folder, Icon, Image, Info, Scissors, Trash } from 'phosphor-react';
import { memo } from 'react';
import { JobProgressEvent, JobReport, useJobInfo } from '@sd/client';
import { ProgressBar } from '@sd/ui';
import { showAlertDialog } from '~/components';
import JobContainer from './JobContainer';

interface JobProps {
	job: JobReport;
	className?: string;
	isChild?: boolean;
	progress: JobProgressEvent | null;
}

const JobIcon: Record<string, Icon> = {
	indexer: Folder,
	thumbnailer: Image,
	file_identifier: Fingerprint,
	file_copier: Copy,
	file_deleter: Trash,
	file_cutter: Scissors,
	object_validator: Fingerprint
};

function Job({ job, className, isChild, progress }: JobProps) {
	const jobData = useJobInfo(job, progress);

	// I don't like sending TSX as a prop due to lack of hot-reload, but it's the only way to get the error log to show up
	if (job.status === 'CompletedWithErrors') {
		const JobError = (
			<pre className="custom-scroll inspector-scroll max-h-[300px] rounded border border-app-darkBox bg-app-darkBox/80 p-3">
				{job.errors_text.map((error, i) => (
					<p
						className="mb-1 w-full overflow-auto whitespace-normal break-words text-sm"
						key={i}
					>
						{error.trim()}
					</p>
				))}
			</pre>
		);
		jobData.textItems?.push([
			{
				text: 'Completed with errors',
				icon: Info,
				onClick: () => {
					showAlertDialog({
						title: 'Error',
						description:
							'The job completed with errors. Please see the error log below for more information. If you need help, please contact support and provide this error.',
						children: JobError
					});
				}
			}
		]);
	}

	return (
		<JobContainer
			className={className}
			name={jobData.name}
			icon={JobIcon[job.name]}
			textItems={
				['Queued'].includes(job.status) ? [[{ text: job.status }]] : jobData.textItems
			}
			isChild={isChild}
		>
			{(jobData.isRunning || jobData.isPaused) && (
				<div className="my-1 ml-1.5 w-[335px]">
					<ProgressBar
						pending={jobData.taskCount == 0}
						value={jobData.completedTaskCount}
						total={jobData.taskCount}
					/>
				</div>
			)}
		</JobContainer>
	);
}

export default memo(Job);
