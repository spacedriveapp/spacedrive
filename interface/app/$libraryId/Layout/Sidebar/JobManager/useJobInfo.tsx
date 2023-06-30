import { Copy, Fingerprint, Folder, Image, Scissors, Trash } from 'phosphor-react';
import { JobProgressEvent, JobReport } from '@sd/client';
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
			name: `${isQueued ? 'Index' : isRunning ? 'Indexing' : 'Indexed'} files  ${indexedPath ? `at ${indexedPath}` : ``
				}`,
			icon: Folder,
			textItems: [
				[
					{
						text: isPaused
							? job.message
							: isRunning && realtimeUpdate?.message
								? realtimeUpdate.message
								: `${comma(output?.total_paths)} ${plural(
									output?.total_paths,
									'path'
								)} discovered`
					}
				]
			]
		},
		thumbnailer: {
			name: `${isQueued ? 'Generate' : isRunning ? 'Generating' : 'Generated'} thumbnails`,
			icon: Image,
			textItems: [
				[
					{
						text:
							output?.thumbnails_created === 0
								? 'None generated'
								: `${completedTaskCount
									? comma(completedTaskCount || 0)
									: comma(output?.thumbnails_created)
								} of ${taskCount} ${plural(taskCount, 'thumbnail')} generated`
					},
					{
						text:
							output?.thumbnails_skipped && `${output?.thumbnails_skipped} already exist`
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
								text: `${comma(output?.total_orphan_paths)} ${plural(
									output?.total_orphan_paths,
									'file'
								)}`
							},
							{
								text: `${comma(output?.total_objects_created)} ${plural(
									output?.total_objects_created,
									'Object'
								)} created`
							},
							{
								text: `${comma(output?.total_objects_linked)} ${plural(
									output?.total_objects_linked,
									'Object'
								)} linked`
							}
						]
					: [{ text: realtimeUpdate?.message }]
			]
		},
		file_copier: {
			name: `${isQueued ? 'Copy' : isRunning ? 'Copying' : 'Copied'} ${isRunning ? completedTaskCount + 1 : completedTaskCount
				} ${isRunning ? `of ${job.task_count}` : ``} ${plural(job.task_count, 'file')}`,
			icon: Copy,
			textItems: [[{ text: job.status }]]
		},
		file_deleter: {
			name: `${isQueued ? 'Delete' : isRunning ? 'Deleting' : 'Deleted'
				} ${completedTaskCount} ${plural(completedTaskCount, 'file')}`,
			icon: Trash,
			textItems: [[{ text: job.status }]]
		},
		file_cutter: {
			name: `${isQueued ? 'Cut' : isRunning ? 'Cutting' : 'Cut'
				} ${completedTaskCount} ${plural(completedTaskCount, 'file')}`,
			icon: Scissors,
			textItems: [[{ text: job.status }]]
		},
		object_validator: {
			name: `${isQueued ? 'Validate' : isRunning ? 'Validating' : 'Validated'} ${!isQueued ? completedTaskCount : ''
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

function comma(x: number) {
	if (!x) return 0;
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}
