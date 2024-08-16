import { TextItems } from '.';
import { formatNumber, humanizeSize, uint32ArrayToBigInt } from '../..';
import {
	JobName,
	JobProgressEvent,
	Report,
	ReportMetadata,
	ReportOutputMetadata
} from '../../core';

interface JobNiceData {
	name: string;
	textItems: TextItems;
	// Job data
	isRunning: boolean;
	isQueued: boolean;
	isPaused: boolean;
	indexedPath?: any;
	taskCount: number;
	completedTaskCount: number;
	meta: ReportMetadata[];
	output: ReportOutputMetadata[];
}

export function useJobInfo(job: Report, realtimeUpdate: JobProgressEvent | null): JobNiceData {
	const isRunning = job.status === 'Running',
		isQueued = job.status === 'Queued',
		isPaused = job.status === 'Paused',
		taskCount = realtimeUpdate?.task_count || job.task_count,
		completedTaskCount = realtimeUpdate?.completed_task_count || job.completed_task_count,
		phase = realtimeUpdate?.phase || job.phase;

	const output: ReportOutputMetadata[] = [];
	let indexedPath: string | undefined;
	for (const metadata of job.metadata) {
		if (metadata.type === 'output') {
			output.push(metadata.metadata);
		}

		if (metadata.type === 'input' && metadata.metadata.type === 'sub_path') {
			indexedPath = metadata.metadata.data;
		}
	}

	const data = {
		isRunning,
		isQueued,
		isPaused,
		taskCount,
		completedTaskCount,
		meta: job.metadata,
		output
	};

	switch (job.name) {
		case 'Indexer': {
			let totalPaths = 0n;
			for (const metadata of output) {
				if (metadata.type === 'indexer') {
					totalPaths = uint32ArrayToBigInt(metadata.data.total_paths);
				}
			}

			return {
				...data,
				indexedPath,
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
									: `${formatNumber(totalPaths)} ${plural(
											totalPaths,
											'path'
										)} discovered`
						}
					]
				]
			};
		}
		case 'MediaProcessor': {
			const generateTexts = () => {
				const parsedOutput = {
					mediaDataExtracted: 0n,
					mediaDataSkipped: 0n,
					thumbnailsGenerated: 0n,
					thumbnailsSkipped: 0n
				};
				for (const metadata of output) {
					if (metadata.type === 'media_processor') {
						const {
							media_data_extracted,
							media_data_skipped,
							thumbnails_generated,
							thumbnails_skipped
						} = metadata.data;

						parsedOutput.mediaDataExtracted = uint32ArrayToBigInt(media_data_extracted);
						parsedOutput.mediaDataSkipped = uint32ArrayToBigInt(media_data_skipped);
						parsedOutput.thumbnailsGenerated =
							uint32ArrayToBigInt(thumbnails_generated);
						parsedOutput.thumbnailsSkipped = uint32ArrayToBigInt(thumbnails_skipped);
					}
				}

				switch (phase) {
					case 'media_data': {
						return [
							{
								text: `${
									completedTaskCount
										? formatNumber(completedTaskCount || 0)
										: formatNumber(parsedOutput.mediaDataExtracted)
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
										: formatNumber(parsedOutput.thumbnailsGenerated)
								} of ${formatNumber(taskCount)} ${plural(
									taskCount,
									'thumbnail'
								)} generated`
							}
						];
					}

					// case 'labels': {
					// 	return [
					// 		{
					// 			text: `Labeled ${
					// 				completedTaskCount
					// 					? formatNumber(completedTaskCount || 0)
					// 					: formatNumber(output?.labels_extracted)
					// 			} of ${formatNumber(taskCount)} ${plural(taskCount, 'file')}`
					// 		}
					// 	];
					// }

					default: {
						// If we don't have a phase set, then we're done

						const totalThumbs =
							parsedOutput.thumbnailsGenerated + parsedOutput.thumbnailsSkipped;
						const totalMediaFiles =
							parsedOutput.mediaDataExtracted + parsedOutput.mediaDataSkipped;

						return totalThumbs === 0n && totalMediaFiles === 0n
							? taskCount === 0
								? [{ text: 'Queued' }]
								: [{ text: 'None processed' }]
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

		case 'FileIdentifier': {
			const parsedOutput = {
				totalOrphanPaths: 0n,
				totalObjectsCreated: 0n,
				totalObjectsLinked: 0n
			};
			for (const metadata of output) {
				if (metadata.type === 'file_identifier') {
					const { total_orphan_paths, total_objects_created, total_objects_linked } =
						metadata.data;

					parsedOutput.totalOrphanPaths = uint32ArrayToBigInt(total_orphan_paths);
					parsedOutput.totalObjectsCreated = uint32ArrayToBigInt(total_objects_created);
					parsedOutput.totalObjectsLinked = uint32ArrayToBigInt(total_objects_linked);
				}
			}

			const generatePausedText = () => {
				switch (phase) {
					case 'searching_orphans': {
						return { text: `Found ${formatNumber(taskCount * 100)} orphans paths` };
					}
					case 'identifying_files': {
						return {
							text: `Identified ${formatNumber(completedTaskCount * 100)} of ${formatNumber(taskCount * 100)} files`
						};
					}
					case 'processing_objects': {
						return {
							text: `Processed ${formatNumber(completedTaskCount * 100)} of ${formatNumber(taskCount * 100)} objects`
						};
					}
					default: {
						return { text: 'No files changed' };
					}
				}
			};

			return {
				...data,
				name: `${isQueued ? 'Extract' : isRunning ? 'Extracting' : 'Extracted'} metadata`,
				textItems: [
					!isRunning
						? parsedOutput.totalOrphanPaths === 0n
							? [generatePausedText()]
							: [
									{
										text: `${formatNumber(parsedOutput.totalOrphanPaths)} ${plural(
											parsedOutput.totalOrphanPaths,
											'file'
										)}`
									},
									{
										text: `${formatNumber(
											parsedOutput.totalObjectsCreated
										)} ${plural(
											parsedOutput.totalObjectsCreated,
											'Object'
										)} created`
									},
									{
										text: `${formatNumber(
											parsedOutput.totalObjectsLinked
										)} ${plural(parsedOutput.totalObjectsLinked, 'Object')} linked`
									}
								]
						: [{ text: addCommasToNumbersInMessage(realtimeUpdate?.message) }]
				]
			};
		}

		case 'Copy':
			return {
				...data,
				name: isQueued
					? `Duplicate ${taskCount} ${plural(taskCount, 'file')}`
					: isRunning
						? `Duplicating ${completedTaskCount}% of ${realtimeUpdate?.info} ${plural(taskCount, 'file')} (${humanizeSize(parseInt(realtimeUpdate?.message || '0'))})`
						: `Duplicated ${taskCount} ${plural(taskCount, 'file')}`,
				textItems: realtimeUpdate
					? [[{ text: realtimeUpdate?.phase }]]
					: [[{ text: job.status }]]
			};

		case 'Delete':
			return {
				...data,
				name: `${
					isQueued ? 'Delete' : isRunning ? 'Deleting' : 'Deleted'
				} ${completedTaskCount} ${plural(completedTaskCount, 'file')}`,
				textItems: [[{ text: job.status }]]
			};
		case 'Move':
			return {
				...data,
				name: `${
					isQueued ? 'Cut' : isRunning ? 'Cutting' : 'Cut'
				} ${completedTaskCount} ${plural(completedTaskCount, 'file')}`,
				textItems: [[{ text: job.status }]]
			};
		case 'FileValidator':
			return {
				...data,
				name: `${isQueued ? 'Validate' : isRunning ? 'Validating' : 'Validated'} ${
					!isQueued ? completedTaskCount : ''
				} ${plural(completedTaskCount, 'file')}`,
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

function plural(count: number | bigint, name?: string) {
	if (count === 1 || count === 1n) return name || '';

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
