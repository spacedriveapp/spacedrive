import type {FootageJob} from '../generated/types';
import {create} from 'zustand';

export interface StartedFootageJob {
	job: FootageJob;
	jobId: string;
}

interface FootageJobState {
	// Jobs started from the app this session, by the path they ran on, so the
	// inspector can follow a job the context menu started.
	byPath: Record<string, StartedFootageJob[]>;
	track: (pathKey: string, job: FootageJob, jobIds: string[]) => void;
}

export const useFootageJobStore = create<FootageJobState>((set) => ({
	byPath: {},
	track: (pathKey, job, jobIds) =>
		set((state) => ({
			byPath: {
				...state.byPath,
				[pathKey]: [
					...(state.byPath[pathKey] ?? []).filter((started) => started.job !== job),
					...jobIds.map((jobId) => ({job, jobId}))
				]
			}
		}))
}));
