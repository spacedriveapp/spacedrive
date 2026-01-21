import { create } from 'zustand';

interface VolumeIndexingState {
	// Maps volume fingerprint to job ID
	volumeToJob: Map<string, string>;

	// Set the job ID for a volume fingerprint
	setVolumeJob: (fingerprint: string, jobId: string) => void;

	// Clear the job for a volume fingerprint
	clearVolumeJob: (fingerprint: string) => void;

	// Get the job ID for a volume fingerprint
	getJobId: (fingerprint: string) => string | undefined;
}

export const useVolumeIndexingStore = create<VolumeIndexingState>((set, get) => ({
	volumeToJob: new Map(),

	setVolumeJob: (fingerprint: string, jobId: string) => {
		set((state) => {
			const newMap = new Map(state.volumeToJob);
			newMap.set(fingerprint, jobId);
			return { volumeToJob: newMap };
		});
	},

	clearVolumeJob: (fingerprint: string) => {
		set((state) => {
			const newMap = new Map(state.volumeToJob);
			newMap.delete(fingerprint);
			return { volumeToJob: newMap };
		});
	},

	getJobId: (fingerprint: string) => {
		return get().volumeToJob.get(fingerprint);
	},
}));
