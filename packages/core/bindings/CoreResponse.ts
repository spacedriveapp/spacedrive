import type { ClientState } from "./ClientState";
import type { Directory } from "./Directory";
import type { LocationResource } from "./LocationResource";
import type { Volume } from "./Volume";

export type CoreResponse = { key: "Success" } | { key: "SysGetVolumes", data: Array<Volume> } | { key: "SysGetLocations", data: LocationResource } | { key: "LibGetExplorerDir", data: Directory } | { key: "ClientGetState", data: ClientState };