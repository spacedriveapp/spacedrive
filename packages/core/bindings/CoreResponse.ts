import type { ClientState } from "./ClientState";
import type { Directory } from "./Directory";
import type { JobResource } from "./JobResource";
import type { LocationResource } from "./LocationResource";
import type { Volume } from "./Volume";

export type CoreResponse = { key: "Success", data: null } | { key: "SysGetVolumes", data: Array<Volume> } | { key: "SysGetLocation", data: LocationResource } | { key: "LibGetExplorerDir", data: Directory } | { key: "ClientGetState", data: ClientState } | { key: "LocCreate", data: LocationResource } | { key: "JobGetRunning", data: Array<JobResource> } | { key: "JobGetHistory", data: Array<JobResource> };