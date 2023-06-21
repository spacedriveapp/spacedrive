import { useExplorerTopBarOptions, useZodRouteParams } from "~/hooks";
import { z } from 'zod'
import { useExplorerSearchParams } from "../Explorer/util";
import { useLibraryQuery } from "@sd/client";
import Explorer from "../Explorer";
import TopBarOptions from "../TopBar/TopBarOptions";
import { TopBarPortal } from "../TopBar/Portal";

const PARAMS = z.object({
	id: z.string()
});

export const Component = () => {
	// const [{ path }] = useExplorerSearchParams();
	const { id: node_id } = useZodRouteParams(PARAMS);

	const locations = useLibraryQuery(["nodes.listLocations", node_id])

	const { explorerViewOptions, explorerControlOptions, explorerToolOptions } =
		useExplorerTopBarOptions();

	return (
		<>
			<TopBarPortal

				right={
					<TopBarOptions
						options={[explorerViewOptions, explorerToolOptions, explorerControlOptions]}
					/>
				}
			/>

			{locations.data && <Explorer items={locations.data} />}
		</>
	)
}


