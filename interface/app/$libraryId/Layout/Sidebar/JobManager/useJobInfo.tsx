import { JobReport } from '@sd/client';
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

export default function useJobInfo(job: JobReport,
): Record<string, JobNiceData> {
	const isRunning = job.status === 'Running',
		// isQueued = job.status === 'Queued',
		indexedPath = job.metadata?.data?.indexed_path,
		taskCount = comma(job.task_count),
		completedTaskCount = comma(job.completed_task_count),
		meta = job.metadata;

	return ({
		indexer: {
			name: isRunning
				? `Indexing files ${indexedPath ? `at ${indexedPath}` : ``}`
				: `Indexed files  ${indexedPath ? `at ${indexedPath}` : ``}`
			,
			icon: Folder,
			textItems: [[
				{ text: isRunning && job.message ? job.message : `${comma(meta?.data?.total_paths)} ${plural(meta?.data?.total_paths, 'path')}` },
			]]
		},
		thumbnailer: {
			name: isRunning
				? 'Generating thumbnails'
				: 'Generated thumbnails'
			,
			icon: Image,
			textItems: [[{ text: `${completedTaskCount} of ${taskCount} ${plural(job.task_count, 'thumbnail')} generated` }]]
		},
		file_identifier: {
			name: `${isRunning
				? 'Extracting metadata'
				: 'Extracted metadata'
				}`,
			icon: Fingerprint,
			textItems: [!isRunning ? [
				{ text: `${comma(meta?.total_orphan_paths)} ${plural(meta?.total_orphan_paths, 'file')}` },
				{ text: `${comma(meta?.total_objects_created)} ${plural(meta?.total_objects_created, 'Object')} created` },
				{ text: `${comma(meta?.total_objects_linked)} ${plural(meta?.total_objects_linked, 'Object')} linked` }
			] : [{ text: job.message }]]
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
