import { Copy, Fingerprint, Folder, Image, Scissors, Trash } from 'phosphor-react';
import { JobProgressEvent, JobReport, formatNumber } from '@sd/client';
import { TextItems } from './JobContainer';

interface JobNiceData {
	name: string;
	icon: React.ForwardRefExoticComponent<any>;
	textItems: TextItems;
}

export default function useJobInfo(
	job: JobReport,
	realtimeUpdate: JobProgressEvent | null
): Record<string, JobNiceData> {
	const isRunning = job.status === 'Running',
		isQueued = job.status === 'Queued',
		isPaused = job.status === 'Paused',
		indexedPath = job.metadata?.data?.location.path,
		taskCount = realtimeUpdate?.task_count || job.task_count,
		completedTaskCount = realtimeUpdate?.completed_task_count || job.completed_task_count,
		meta = job.metadata,
		output = meta?.output?.run_metadata;

	return {
		indexer: {
			name: `${isQueued ? 'Index' : isRunning ? 'Indexing' : 'Indexed'} files  ${
				indexedPath ? `at ${indexedPath}` : ``
			}`,
			icon: Folder,
			textItems: [
				[
					{
						text: isPaused
							? job.message
							: isRunning && realtimeUpdate?.message
							? realtimeUpdate.message
							: `${formatNumber(output?.total_paths)} ${plural(
									output?.total_paths,
									'path'
							  )} discovered`
					}
				]
			]
		},
		media_processor: {
			name: `${isQueued ? 'Process' : isRunning ? 'Processing' : 'Processed'} media files`,
			icon: Image,
			textItems: [
				[
					{
						text:
							output?.thumbnails_created === 0
								? 'None processed'
								: `${
										completedTaskCount
											? formatNumber(completedTaskCount || 0)
											: formatNumber(output?.thumbnails_created)
								  } of ${taskCount} ${plural(taskCount, 'media file')} processed`
					},
					{
						text:
							output?.thumbnails_skipped &&
							`${output?.thumbnails_skipped} already exist`
					}
				]
			]
		},
		file_identifier: {
			name: `${isQueued ? 'Extract' : isRunning ? 'Extracting' : 'Extracted'} metadata`,
			icon: Fingerprint,
			textItems: [
				!isRunning
					? output?.total_orphan_paths === 0
						? [{ text: 'No files changed' }]
						: [
								{
									text: `${formatNumber(output?.total_orphan_paths)} ${plural(
										output?.total_orphan_paths,
										'file'
									)}`
								},
								{
									text: `${formatNumber(output?.total_objects_created)} ${plural(
										output?.total_objects_created,
										'Object'
									)} created`
								},
								{
									text: `${formatNumber(output?.total_objects_linked)} ${plural(
										output?.total_objects_linked,
										'Object'
									)} linked`
								}
						  ]
					: [{ text: addCommasToNumbersInMessage(realtimeUpdate?.message) }]
			]
		},
		file_copier: {
			name: `${isQueued ? 'Copy' : isRunning ? 'Copying' : 'Copied'} ${
				isRunning ? completedTaskCount + 1 : completedTaskCount
			} ${isRunning ? `of ${job.task_count}` : ``} ${plural(job.task_count, 'file')}`,
			icon: Copy,
			textItems: [[{ text: job.status }]]
		},
		file_deleter: {
			name: `${
				isQueued ? 'Delete' : isRunning ? 'Deleting' : 'Deleted'
			} ${completedTaskCount} ${plural(completedTaskCount, 'file')}`,
			icon: Trash,
			textItems: [[{ text: job.status }]]
		},
		file_cutter: {
			name: `${
				isQueued ? 'Cut' : isRunning ? 'Cutting' : 'Cut'
			} ${completedTaskCount} ${plural(completedTaskCount, 'file')}`,
			icon: Scissors,
			textItems: [[{ text: job.status }]]
		},
		object_validator: {
			name: `${isQueued ? 'Validate' : isRunning ? 'Validating' : 'Validated'} ${
				!isQueued ? completedTaskCount : ''
			} ${plural(completedTaskCount, 'object')}`,
			icon: Fingerprint,
			textItems: [[{ text: job.status }]]
		}
	};
}

function plural(count: number, name?: string) {
	if (count === 1) {
		return name || '';
	}
	return `${name || ''}s`;
}

function addCommasToNumbersInMessage(input?: string): string {
	if (!input) return '';
	// use regular expression to split on numbers
	const parts = input.split(/(\d+)/);
	for (let i = 0; i < parts.length; i++) {
		// if a part is a number, convert it to number and pass to the comma function
		if (!isNaN(Number(parts[i]))) {
			const part = parts[i];
			if (part) parts[i] = formatNumber(parseInt(part));
		}
	}
	// join the parts back together
	return parts.join('');
}
