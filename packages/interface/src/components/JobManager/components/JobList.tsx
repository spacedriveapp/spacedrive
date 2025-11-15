import { AnimatePresence, motion } from "framer-motion";
import type { JobListItem } from "../types";
import { JobCard } from "./JobCard";
import { EmptyState } from "./EmptyState";

interface JobListProps {
  jobs: JobListItem[];
  onPause?: (jobId: string) => void;
  onResume?: (jobId: string) => void;
}

export function JobList({ jobs, onPause, onResume }: JobListProps) {
  if (jobs.length === 0) {
    return <EmptyState />;
  }

  return (
    <div className="flex flex-col gap-2 p-2">
      <AnimatePresence mode="popLayout">
        {jobs.map((job) => (
          <motion.div
            key={job.id}
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, x: -10 }}
            transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
          >
            <JobCard job={job} onPause={onPause} onResume={onResume} />
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
