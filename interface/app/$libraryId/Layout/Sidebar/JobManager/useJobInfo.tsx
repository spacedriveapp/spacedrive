import { JobProgressEvent, JobReport } from '@sd/client';
import {
	Fingerprint,
	Folder,
	Image
} from 'phosphor-react';
import { TextItems } from './JobContainer';

interface JobNiceData {
	name: string;
	icon: React.ForwardRefExoticComponent<any>;
	textItems: TextItems;
}

export default function useJobInfo(job: JobReport, realtimeUpdate: JobProgressEvent | null
): Record<string, JobNiceData> {
	const isRunning = job.status === 'Running',
		isQueued = job.status === 'Queued',
		indexedPath = job.metadata?.data?.indexed_path,
		taskCount = realtimeUpdate?.task_count ? comma(realtimeUpdate?.task_count || 0) : comma(job.task_count),
		meta = job.metadata;


	return ({
		indexer: {
			name: `${isQueued ? "Index" : isRunning ? "Indexing" : "Indexed"} files  ${indexedPath ? `at ${indexedPath}` : ``}`,
			icon: Folder,
			textItems: [[
				{ text: isRunning && realtimeUpdate?.message ? realtimeUpdate.message : `${comma(meta?.data?.total_paths)} ${plural(meta?.data?.total_paths, 'path')}` },
			]]
		},
		thumbnailer: {
			name: `${isQueued ? "Generate" : isRunning ? "Generating" : "Generated"} thumbnails`,
			icon: Image,
			textItems: [[{ text: meta?.thumbnails_created === 0 ? "None generated" : `${realtimeUpdate?.completed_task_count ? comma(realtimeUpdate?.completed_task_count || 0) : comma(meta?.thumbnails_created)} of ${taskCount} ${plural(job.task_count, 'thumbnail')} generated` }, { text: meta?.thumbnails_skipped && `${meta?.thumbnails_skipped} already exist` }]]
		},
		file_identifier: {
			name: `${isQueued ? "Extract" : isRunning ? "Extracting" : "Extracted"} metadata`,
			icon: Fingerprint,
			textItems: [!isRunning ? meta?.total_orphan_paths === 0 ? [{ text: "No files changed" }] : [
				{ text: `${comma(meta?.total_orphan_paths)} ${plural(meta?.total_orphan_paths, 'file')}` },
				{ text: `${comma(meta?.total_objects_created)} ${plural(meta?.total_objects_created, 'Object')} created` },
				{ text: `${comma(meta?.total_objects_linked)} ${plural(meta?.total_objects_linked, 'Object')} linked` }
			] : [{ text: realtimeUpdate?.message }]]
		},
		// Repeat the similar pattern for all subtext fields
	})
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
