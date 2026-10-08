import { useMemo } from "react";
import { useCoreQuery } from "../contexts/SpacedriveContext";
import {
	indexExtensionKinds,
	type ExtensionKinds,
} from "../components/QuickPreview/renderers";

/**
 * The kinds of every loaded extension, keyed by kind id. This is the client's
 * only source of extension kinds: a stored kind name that is not here belongs
 * to an unloaded extension and resolves to its parent kind.
 */
export function useExtensionKinds(): ExtensionKinds {
	const { data } = useCoreQuery(
		{ type: "extensions.list", input: {} },
		{ staleTime: 30_000 },
	);
	return useMemo(() => indexExtensionKinds(data), [data]);
}
