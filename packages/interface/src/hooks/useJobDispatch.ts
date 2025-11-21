import {useLibraryMutation} from "../context";

export interface JobConfig {
	job_type: "thumbnail" | "ocr" | "speech_to_text" | "thumbstrip" | "proxy";
	file_ids: string[];
	params?: Record<string, any>;
}

/**
 * Hook for dispatching media processing jobs from the context menu
 * 
 * Provides a simple interface to trigger thumbnail generation, OCR,
 * speech-to-text transcription, thumbstrip generation, and proxy creation.
 * 
 * @example
 * ```tsx
 * const { runJob, isDispatching } = useJobDispatch();
 * 
 * // Generate thumbnails with blurhash
 * await runJob("thumbnail", {
 *   file_ids: [file.id],
 *   generate_blurhash: true
 * });
 * 
 * // Run OCR on images
 * await runJob("ocr", {
 *   file_ids: selectedFiles.map(f => f.id)
 * });
 * ```
 */
export function useJobDispatch() {
	const dispatchJob = useLibraryMutation("jobs.dispatch");

	const runJob = async (jobType: string, params: any) => {
		try {
			console.log(`Dispatching ${jobType} job:`, params);
			const result = await dispatchJob.mutateAsync({
				job_type: jobType,
				...params,
			});

			console.log(`${jobType} job dispatched:`, result);

			// TODO: Show toast notification
			// toast.success(`${jobType} job started`);

			return result;
		} catch (err) {
			console.error(`Failed to dispatch ${jobType} job:`, err);
			// TODO: Show error toast
			// toast.error(`Failed to start ${jobType} job`);
			throw err;
		}
	};

	return {runJob, isDispatching: dispatchJob.isPending};
}




