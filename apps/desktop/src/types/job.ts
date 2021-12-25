export interface Job {
  // A job is used to define a task for the software to complete
  // These are intended to be stored in memory, or not persisted permanently
  object_ids: string[]; // array of object ids that concern this job
  type: JobType;
  created_at: Date;
  completed_at: Date;
  canceled_at: Date;
  parent_job_id: string;
}

export enum JobType {
  SCAN,
  IMPORT,
  ENCRYPT,
  DECRYPT,
  COPY,
  MOVE,
  DELETE,
  RENDER_VIDEO,
  RENDER_IMAGE
}
