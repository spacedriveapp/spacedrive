import type { File } from "./File";
import type { JobMetadata } from "./JobMetadata";
import type { LocationResource } from "./LocationResource";

export type CoreResource = "Client" | "Library" | { Location: LocationResource } | { File: File } | { Job: JobMetadata } | "Tag";