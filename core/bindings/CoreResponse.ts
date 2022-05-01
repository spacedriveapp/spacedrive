import type { ClientState } from "./ClientState";
import type { DirectoryWithContents } from "./DirectoryWithContents";
import type { JobReport } from "./JobReport";
import type { LocationResource } from "./LocationResource";
import type { Statistics } from "./Statistics";
import type { Volume } from "./Volume";

export type CoreResponse = { key: "Success", data: null } | { key: "SysGetVolumes", data: Array<Volume> } | { key: "SysGetLocation", data: LocationResource } | { key: "SysGetLocations", data: Array<LocationResource> } | { key: "LibGetExplorerDir", data: DirectoryWithContents } | { key: "ClientGetState", data: ClientState } | { key: "LocCreate", data: LocationResource } | { key: "JobGetRunning", data: Array<JobReport> } | { key: "JobGetHistory", data: Array<JobReport> } | { key: "GetLibraryStatistics", data: Statistics };
