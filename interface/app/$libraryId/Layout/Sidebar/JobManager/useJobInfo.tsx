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
	const isRunning = job.status === 'Running';
	const isQueued = job.status === 'Queued';
	const indexedPath = job.metadata?.data?.indexed_path;
	const taskCount = comma(job.task_count);
	const completedTaskCount = comma(job.completed_task_count);
	// const fileCount = plural(job.task_count, 'file');
	const meta = job.metadata;

	return ({
		indexer: {
			name: isRunning || isQueued
				? `Indexing files ${indexedPath && `at ${indexedPath}`}`
				: `Indexed files  ${indexedPath && `at ${indexedPath}`}`
			,
			icon: Folder,
			textItems: [[
				{ text: `${comma(job.completed_task_count)} of ${comma(job.task_count)} ${plural(job.task_count, 'task')}` },
				{ text: `${comma(meta?.total_indexed_directories)} ${plural(meta?.total_indexed_directories, 'folder')}` },
				{ text: `${comma(meta?.total_indexed_files)} ${plural(meta?.total_indexed_files, 'file')}` },
			]]
		},
		thumbnailer: {
			name: isRunning || isQueued
				? 'Generating thumbnails'
				: 'Generated thumbnails'
			,
			icon: Image,
			textItems: [[{ text: `${completedTaskCount} of ${taskCount} ${plural(job.task_count, 'thumbnail')} generated` }]]
		},
		file_identifier: {
			name: `${isRunning || isQueued
				? 'Extracting metadata'
				: 'Extracted metadata'
				}`,
			icon: Fingerprint,
			textItems: [[
				{ text: `${comma(meta?.total_orphan_paths)} ${plural(meta?.total_orphan_paths, 'file')}` },
				{ text: `${comma(meta?.total_objects_created)} ${plural(meta?.total_objects_created, 'Object')} created` },
				{ text: `${comma(meta?.total_objects_linked)} ${plural(meta?.total_objects_linked, 'Object')} linked` }
			]]
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
