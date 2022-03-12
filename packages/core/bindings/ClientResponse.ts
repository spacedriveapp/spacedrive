import type { Volume } from "./Volume";

export type ClientResponse = { key: "sys_get_volumes", data: Array<Volume> };