import {
	Copy,
	Fingerprint,
	Folder,
	Icon,
	Image,
	Info,
	Lightning,
	Scissors,
	Trash
} from '@phosphor-icons/react';
import { memo } from 'react';
import { JobName, JobProgressEvent, Report, useJobInfo } from '@sd/client';
import { ProgressBar } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { useLocale } from '~/hooks';

import JobContainer from './JobContainer';

interface JobProps {
	job: Report;
	className?: string;
	isChild?: boolean;
	progress: JobProgressEvent | null;
	eta: number;
}

export const JobIcon: Record<JobName, Icon> = {
	Indexer: Folder,
	MediaProcessor: Image,
	FileIdentifier: Fingerprint,
	Copy: Copy,
	Delete: Trash,
	Erase: Trash,
	Move: Scissors,
	FileValidator: Fingerprint
};

// Jobs like deleting and copying files do not have simplied job names
// so we need to use the metadata to display an icon
const MetaDataJobIcon = {
	deleter: Trash,
	copier: Copy
};

function Job({ job, className, isChild, progress, eta }: JobProps) {
	const jobData = useJobInfo(job, progress);
	const { t } = useLocale();
	// I don't like sending TSX as a prop due to lack of hot-reload, but it's the only way to get the error log to show up
	if (job.status === 'CompletedWithErrors') {
		const JobError = (
			<pre className="custom-scroll inspector-scroll max-h-[300px] rounded border border-app-darkBox bg-app-darkBox/80 p-3">
				{job.non_critical_errors.map((error, i) => (
					<p
						className="mb-1 w-full overflow-auto whitespace-normal break-words text-sm"
						key={i}
					>
						{/* TODO: Report errors in a nicer way */}
						{JSON.stringify(error)}
					</p>
				))}
			</pre>
		);
		jobData.textItems?.push([
			{
				text: t('completed_with_errors'),
				icon: Info,
				onClick: () => {
					showAlertDialog({
						title: t('error'),
						description: t('job_error_description'),
						children: JobError
					});
				}
			}
		]);
	}

	let jobIcon = Lightning;
	if (job.name in JobIcon) {
		jobIcon = JobIcon[job.name];
	} else {
		const meta = [...jobData.meta, ...jobData.output].find(
			(meta) => meta.type in MetaDataJobIcon
		);
		if (meta) {
			jobIcon = MetaDataJobIcon[meta.type as keyof typeof MetaDataJobIcon];
		}
	}

	return (
		<JobContainer
			className={className}
			name={jobData.name}
			icon={jobIcon}
			eta={eta}
			status={job.status}
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
