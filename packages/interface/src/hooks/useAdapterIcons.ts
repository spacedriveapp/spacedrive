import { useLibraryQuery } from "../contexts/SpacedriveContext";
import { useCallback } from "react";

/**
 * Hook that provides adapter icon lookup by adapter ID.
 * Fetches the adapters list once and caches via React Query.
 */
export function useAdapterIcons() {
	const { data: adapters } = useLibraryQuery({
		type: "adapters.list",
		input: {},
	});

	const getIcon = useCallback(
		(adapterId: string): string | null => {
			if (!adapters) return null;
			return adapters.find((a) => a.id === adapterId)?.icon_svg ?? null;
		},
		[adapters],
	);

	return { getIcon };
}
