import type { JobStatus } from './JobStatus';

export interface JobReport {
  id: string;
  date_created: string;
  date_modified: string;
  status: JobStatus;
  task_count: number;
  completed_task_count: number;
  message: string;
  seconds_elapsed: string;
}
