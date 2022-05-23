import type { DirectoryWithContents } from "./DirectoryWithContents";
import type { JobReport } from "./JobReport";
import type { LocationResource } from "./LocationResource";
import type { NodeState } from "./NodeState";
import type { Statistics } from "./Statistics";
import type { Volume } from "./Volume";

export type CoreResponse = { key: "Success", data: null } | { key: "SysGetVolumes", data: Array<Volume> } | { key: "SysGetLocation", data: LocationResource } | { key: "SysGetLocations", data: Array<LocationResource> } | { key: "LibGetExplorerDir", data: DirectoryWithContents } | { key: "NodeGetState", data: NodeState } | { key: "LocCreate", data: LocationResource } | { key: "JobGetRunning", data: Array<JobReport> } | { key: "JobGetHistory", data: Array<JobReport> } | { key: "GetLibraryStatistics", data: Statistics };