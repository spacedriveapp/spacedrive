import type { JobStatus } from "./JobStatus";

export interface JobReport { id: string, date_created: string, date_modified: string, status: JobStatus, task_count: bigint, completed_task_count: bigint, message: string, seconds_elapsed: string, }