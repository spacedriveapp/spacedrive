import type { JobStatus } from "@sd/ts-client/generated/types";
import {
  MagnifyingGlass,
  Image,
  Files,
  FolderOpen,
  Database,
  HardDrive,
  FolderPlus,
  Sparkle,
  CheckCircle,
} from "@phosphor-icons/react";
import { motion, AnimatePresence } from "framer-motion";
import type { JobListItem } from "../types";

interface JobStatusIndicatorProps {
  job: JobListItem;
}

// Phase icons for indexer
const INDEXER_PHASES = [
  { name: "Discovery", icon: MagnifyingGlass },
  { name: "Processing", icon: Files },
  { name: "Content Identification", icon: Sparkle },
  { name: "Finalizing", icon: CheckCircle },
];

// Phase icons for thumbnail generation
const THUMBNAIL_PHASES = [
  { name: "Discovery", icon: MagnifyingGlass },
  { name: "Processing", icon: Image },
  { name: "Cleanup", icon: CheckCircle },
];

// Get phases for a job type
function getJobPhases(jobName: string) {
  if (jobName === "indexer") {
    return INDEXER_PHASES;
  }
  if (jobName === "thumbnail_generation") {
    return THUMBNAIL_PHASES;
  }
  return null;
}

// Map job names to default icons (when no phases)
function getJobIcon(jobName: string) {
  if (jobName === "indexer") {
    return MagnifyingGlass;
  }
  if (jobName === "thumbnail_generation") {
    return Image;
  }
  if (jobName.includes("copy") || jobName.includes("move")) {
    return Files;
  }
  if (jobName.includes("delete")) {
    return FolderOpen;
  }
  return Database;
}

export function JobStatusIndicator({ job }: JobStatusIndicatorProps) {
  const phases = getJobPhases(job.name);
  const currentPhase = job.current_phase;

  // If job has phases and we know the current phase, show carousel
  if (phases && currentPhase) {
    const currentIndex = phases.findIndex(p => p.name === currentPhase);

    // Show 3 icons: previous, current, next
    const PrevIcon = phases[currentIndex - 1]?.icon;
    const CurrentIcon = phases[currentIndex]?.icon;
    const NextIcon = phases[currentIndex + 1]?.icon;

    return (
      <div className="flex-shrink-0 flex items-center justify-center w-10 h-full overflow-hidden">
        <div className="relative h-full w-full flex flex-col items-center justify-center">
          <AnimatePresence mode="popLayout">
            <motion.div
              key={currentIndex}
              className="flex flex-col items-center gap-2"
              initial={{ y: 20, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              exit={{ y: -20, opacity: 0 }}
              transition={{ duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
            >
              {/* Previous phase (dimmed) */}
              {PrevIcon && (
                <PrevIcon
                  size={12}
                  weight="duotone"
                  className="text-ink-faint opacity-30"
                />
              )}

              {/* Current phase (highlighted) */}
              {CurrentIcon && (
                <CurrentIcon
                  size={16}
                  weight="duotone"
                  className="text-ink-faint"
                />
              )}

              {/* Next phase (dimmed) */}
              {NextIcon && (
                <NextIcon
                  size={12}
                  weight="duotone"
                  className="text-ink-faint opacity-30"
                />
              )}
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    );
  }

  // No phases - show single icon
  const Icon = getJobIcon(job.name);
  return (
    <div className="flex-shrink-0 flex items-center justify-center w-10 h-full">
      <Icon size={16} weight="duotone" className="text-ink-faint" />
    </div>
  );
}
