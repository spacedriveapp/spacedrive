import { TextItems } from '.';
import { formatNumber } from '../..';
import { JobProgressEvent, JobReport } from '../../core';

interface JobNiceData {
	name: string;
	textItems: TextItems;
	// Job data
	isRunning: boolean;
	isQueued: boolean;
	isPaused: boolean;
	indexedPath: any;
	taskCount: number;
	completedTaskCount: number;
	meta: any;
	output: any;
}

export function useJobInfo(job: JobReport, realtimeUpdate: JobProgressEvent | null): JobNiceData {
	const isRunning = job.status === 'Running',
		isQueued = job.status === 'Queued',
		isPaused = job.status === 'Paused',
		indexedPath = (job.metadata?.data as any)?.location.path,
		taskCount = realtimeUpdate?.task_count || job.task_count,
		completedTaskCount = realtimeUpdate?.completed_task_count || job.completed_task_count,
		phase = realtimeUpdate?.phase,
		meta = job.metadata,
		output = (meta?.output as any)?.run_metadata;

	const data = {
		isRunning,
		isQueued,
		isPaused,
		indexedPath,
		taskCount,
		completedTaskCount,
		meta,
		output
	};

	switch (job.name) {
		case 'indexer':
			return {
				...data,
				name: `${isQueued ? 'Index' : isRunning ? 'Indexing' : 'Indexed'} files  ${
					indexedPath ? `at ${indexedPath}` : ``
				}`,
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
			};
		case 'media_processor': {
			const generateTexts = () => {
				switch (phase) {
					case 'media_data': {
						return [
							{
								text: `${
									completedTaskCount
										? formatNumber(completedTaskCount || 0)
										: formatNumber(output?.media_data?.extracted)
								} of ${formatNumber(taskCount)} ${plural(
									taskCount,
									'media file'
								)} processed`
							}
						];
					}

					case 'thumbnails': {
						return [
							{
								text: `${
									completedTaskCount
										? formatNumber(completedTaskCount || 0)
										: formatNumber(output?.thumbs_processed)
								} of ${formatNumber(taskCount)} ${plural(
									taskCount,
									'thumbnail'
								)} generated`
							}
						];
					}

					case 'labels': {
						return [
							{
								text: `Labeled ${
									completedTaskCount
										? formatNumber(completedTaskCount || 0)
										: formatNumber(output?.labels_extracted)
								} of ${formatNumber(taskCount)} ${plural(taskCount, 'file')}`
							}
						];
					}

					default: {
						// If we don't have a phase set, then we're done

						const totalThumbs = output?.thumbs_processed || 0;
						const totalMediaFiles =
							output?.media_data?.extracted || 0 + output?.media_data?.skipped || 0;

						return totalThumbs === 0 && totalMediaFiles === 0
							? [{ text: 'None processed' }]
							: [
									{
										text: `Extracted ${formatNumber(totalMediaFiles)} ${plural(
											totalMediaFiles,
											'media file'
										)}`
									},
									{
										text: `Generated ${formatNumber(totalThumbs)} ${plural(
											totalThumbs,
											'thumb'
										)}`
									}
								];
					}
				}
			};

			return {
				...data,
				name: `${
					isQueued ? 'Process' : isRunning ? 'Processing' : 'Processed'
				} media files`,
				textItems: [generateTexts()]
			};
		}

		case 'file_identifier':
			return {
				...data,
				name: `${isQueued ? 'Extract' : isRunning ? 'Extracting' : 'Extracted'} metadata`,
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
										text: `${formatNumber(
											output?.total_objects_created
										)} ${plural(
											output?.total_objects_created,
											'Object'
										)} created`
									},
									{
										text: `${formatNumber(
											output?.total_objects_linked
										)} ${plural(output?.total_objects_linked, 'Object')} linked`
									}
								]
						: [{ text: addCommasToNumbersInMessage(realtimeUpdate?.message) }]
				]
			};
		case 'file_copier':
			return {
				...data,
				name: `${isQueued ? 'Copy' : isRunning ? 'Copying' : 'Copied'} ${
					isRunning ? completedTaskCount + 1 : completedTaskCount
				} ${isRunning ? `of ${job.task_count}` : ``} ${plural(job.task_count, 'file')}`,
				textItems: [[{ text: job.status }]]
			};
		case 'file_deleter':
			return {
				...data,
				name: `${
					isQueued ? 'Delete' : isRunning ? 'Deleting' : 'Deleted'
				} ${completedTaskCount} ${plural(completedTaskCount, 'file')}`,
				textItems: [[{ text: job.status }]]
			};
		case 'file_cutter':
			return {
				...data,
				name: `${
					isQueued ? 'Cut' : isRunning ? 'Cutting' : 'Cut'
				} ${completedTaskCount} ${plural(completedTaskCount, 'file')}`,
				textItems: [[{ text: job.status }]]
			};
		case 'object_validator':
			return {
				...data,
				name: `${isQueued ? 'Validate' : isRunning ? 'Validating' : 'Validated'} ${
					!isQueued ? completedTaskCount : ''
				} ${plural(completedTaskCount, 'object')}`,
				textItems: [[{ text: job.status }]]
			};
		default:
			return {
				...data,
				name: job.name,
				textItems: [[{ text: job.status.replace(/([A-Z])/g, ' $1').trim() }]]
			};
	}
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
