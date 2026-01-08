import { AnimatePresence, motion } from "framer-motion";
import type { JobListItem } from "../types";
import { EmptyState } from "./EmptyState";
import { JobCard } from "./JobCard";

interface JobListProps {
  jobs: JobListItem[];
  onPause?: (jobId: string) => void;
  onResume?: (jobId: string) => void;
  onCancel?: (jobId: string) => void;
}

export function JobList({ jobs, onPause, onResume, onCancel }: JobListProps) {
  if (jobs.length === 0) {
    return <EmptyState />;
  }

  return (
    <div className="flex flex-col gap-2 p-2">
      <AnimatePresence mode="popLayout">
        {jobs.map((job) => (
          <motion.div
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, x: -10 }}
            initial={{ opacity: 0, y: -10 }}
            key={job.id}
            transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
          >
            <JobCard
              job={job}
              onCancel={onCancel}
              onPause={onPause}
              onResume={onResume}
            />
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
