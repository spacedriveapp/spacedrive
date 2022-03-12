import type { Volume } from "./Volume";

export type ClientResponse = { key: "SysGetVolumes", data: Array<Volume> };