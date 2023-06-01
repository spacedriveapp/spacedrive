import { JobReport } from '@sd/client';
import {
	Camera,
	Copy,
	Eye,
	Fingerprint,
	Folder,
	LockSimple,
	LockSimpleOpen,
	Scissors,
	Trash,
	TrashSimple
} from 'phosphor-react';

interface JobNiceData {
	name: string;
	icon: React.ForwardRefExoticComponent<any>;
	subtext: string;
}

export default function useJobInfo(job: JobReport,
): Record<string, JobNiceData> {

	return ({
		indexer: {
			name: job.metadata?.data?.indexed_path
				? `Indexed paths at ${job.metadata?.data?.indexed_path}`
				: `Indexing paths at ${job.metadata?.data?.indexed_path}`,
			icon: Folder,
			subtext: `${numberWithCommas(job.metadata?.data?.total_paths || 0)} ${appendPlural(job.task_count, 'path')} discovered`
		},
		thumbnailer: {
			name: `${job.status === 'Running' || job.status === 'Queued'
				? 'Generating thumbnails'
				: 'Generated thumbnails'
				}`,
			icon: Camera,
			subtext: `${numberWithCommas(job.completed_task_count)} of ${numberWithCommas(
				job.task_count
			)} ${appendPlural(job.task_count, 'thumbnail')} generated`
		},
		file_identifier: {
			name: `${job.status === 'Running' || job.status === 'Queued'
				? 'Extracting metadata'
				: 'Extracted metadata'
				}`,
			icon: Eye,
			subtext:
				`${numberWithCommas(job.metadata?.total_orphan_paths)} ${appendPlural(
					job.metadata?.total_orphan_paths,
					'file',
				)} - ${numberWithCommas(job.metadata?.total_objects_created)} ${appendPlural(job.metadata?.total_objects_created, 'Object')} created - ${numberWithCommas(job.metadata?.total_objects_linked)} ${appendPlural(job.metadata?.total_objects_linked, 'Object')} linked`
		},
		object_validator: {
			name: `Generated full object hashes`,
			icon: Fingerprint,
			subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job.task_count, 'object')}`
		},
		file_encryptor: {
			name: `Encrypted`,
			icon: LockSimple,
			subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job.task_count, 'file')}`
		},
		file_decryptor: {
			name: `Decrypted`,
			icon: LockSimpleOpen,
			subtext: `${numberWithCommas(job.task_count)}${appendPlural(job.task_count, 'file')}`
		},
		file_eraser: {
			name: `Securely erased`,
			icon: TrashSimple,
			subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job.task_count, 'file')}`
		},
		file_deleter: {
			name: `Deleted`,
			icon: Trash,
			subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job.task_count, 'file')}`
		},
		file_copier: {
			name: `Copied`,
			icon: Copy,
			subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job.task_count, 'file')}`
		},
		file_cutter: {
			name: `Moved`,
			icon: Scissors,
			subtext: `${numberWithCommas(job.task_count)} ${appendPlural(job.task_count, 'file')}`
		}
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
