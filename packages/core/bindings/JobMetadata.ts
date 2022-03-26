import type { JobStatus } from "./JobStatus";

export interface JobMetadata { id: bigint, client_id: bigint, date_created: string, date_modified: string, status: JobStatus, task_count: bigint, completed_task_count: bigint, message: string, }