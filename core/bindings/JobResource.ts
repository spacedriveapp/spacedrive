import type { JobAction } from "./JobAction";
import type { JobStatus } from "./JobStatus";

export interface JobResource { id: bigint, client_id: bigint, action: JobAction, status: JobStatus, percentage_complete: bigint, task_count: bigint, message: string, completed_task_count: bigint, date_created: string, date_modified: string, }