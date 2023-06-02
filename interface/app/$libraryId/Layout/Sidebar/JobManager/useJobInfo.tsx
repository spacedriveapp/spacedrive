import { JobReport } from '@sd/client';
import {
	Copy,
	Fingerprint,
	Folder,
	Image,
	LockSimple,
	LockSimpleOpen,
	Scissors,
	Trash,
	TrashSimple
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
	const taskCount = numberWithCommas(job.task_count);
	const completedTaskCount = numberWithCommas(job.completed_task_count);
	const fileCount = appendPlural(job.task_count, 'file');

	return ({
		thumbnailer: {
			name: `${isRunning || isQueued
				? 'Generating thumbnails'
				: 'Generated thumbnails'
				}`,
			icon: Image,
			textItems: [[{ text: `${completedTaskCount} of ${taskCount} ${appendPlural(job.task_count, 'thumbnail')} generated` }]]
		},
		file_identifier: {
			name: `${isRunning || isQueued
				? 'Extracting metadata'
				: 'Extracted metadata'
				}`,
			icon: Fingerprint,
			textItems: [[
				{ text: `${numberWithCommas(job.metadata?.total_orphan_paths)} ${appendPlural(job.metadata?.total_orphan_paths, 'file')}` },
				{ text: `${numberWithCommas(job.metadata?.total_objects_created)} ${appendPlural(job.metadata?.total_objects_created, 'Object')} created` },
				{ text: `${numberWithCommas(job.metadata?.total_objects_linked)} ${appendPlural(job.metadata?.total_objects_linked, 'Object')} linked` }
			]]
		},
		indexer: {
			name: `${isRunning || isQueued
				? 'Indexing files'
				: 'Indexed files'
				}`,
			icon: Folder,
			textItems: [[
				{ text: `${numberWithCommas(job.completed_task_count)} of ${numberWithCommas(job.task_count)} ${appendPlural(job.task_count, 'task')}` },
				{ text: `${numberWithCommas(job.metadata?.total_indexed_directories)} ${appendPlural(job.metadata?.total_indexed_directories, 'folder')}` },
				{ text: `${numberWithCommas(job.metadata?.total_indexed_files)} ${appendPlural(job.metadata?.total_indexed_files, 'file')}` },
			]]
		}

		// Repeat the similar pattern for all subtext fields
	})
}



function appendPlural(count: number, name?: string) {
	if (count === 1) {
		return name || '';
	}
	return `${name || ''}s`;
}

function numberWithCommas(x: number) {
	if (!x) return 0;
	return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}
