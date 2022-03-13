import type { LocationResource } from "./LocationResource";
import type { Volume } from "./Volume";

export type CoreResponse = { key: "Success" } | { key: "SysGetVolumes", data: Array<Volume> } | { key: "SysGetLocations", data: LocationResource };