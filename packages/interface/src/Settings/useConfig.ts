import { useCallback } from "react";
import {
	useCoreMutation,
	useCoreQuery,
	useLibraryMutation,
	useLibraryQuery,
	useSpacedriveClient,
} from "../contexts/SpacedriveContext";
import type {
	UpdateAppConfigInput,
	UpdateAppConfigOutput,
	UpdateLibraryConfigInput,
} from "@sd/ts-client";

/**
 * App-scoped config with a patch-and-refetch writer. Settings commit as they
 * change, so callers pass only the fields they touched.
 */
export function useAppConfig() {
	const query = useCoreQuery({ type: "config.app.get", input: null });
	const mutation = useCoreMutation("config.app.update", {
		onSuccess: () => {
			query.refetch();
		},
	});

	const update = useCallback(
		(patch: UpdateAppConfigInput): Promise<UpdateAppConfigOutput> =>
			mutation.mutateAsync(patch),
		[mutation],
	);

	return {
		config: query.data,
		isLoading: query.isLoading,
		isSaving: mutation.isPending,
		update,
	};
}

/**
 * Config for the currently selected library. `libraryId` is null when no
 * library is open, in which case the query stays idle.
 */
export function useLibraryConfig() {
	const client = useSpacedriveClient();
	const libraryId = client.getCurrentLibraryId();

	const query = useLibraryQuery(
		{ type: "config.library.get", input: null },
		{ enabled: !!libraryId },
	);
	const mutation = useLibraryMutation("config.library.update", {
		onSuccess: () => {
			query.refetch();
		},
	});

	const update = useCallback(
		(patch: UpdateLibraryConfigInput) => mutation.mutateAsync(patch),
		[mutation],
	);

	return {
		libraryId,
		config: query.data,
		isLoading: query.isLoading,
		isSaving: mutation.isPending,
		update,
	};
}
