import {
  CheckCircle,
  Database,
  Files,
  FolderOpen,
  Image,
  MagnifyingGlass,
  Sparkle,
} from "@phosphor-icons/react";
import { AnimatePresence, motion } from "framer-motion";
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
    const currentIndex = phases.findIndex((p) => p.name === currentPhase);

    // Show 3 icons: previous, current, next
    const PrevIcon = phases[currentIndex - 1]?.icon;
    const CurrentIcon = phases[currentIndex]?.icon;
    const NextIcon = phases[currentIndex + 1]?.icon;

    return (
      <div className="flex h-full w-10 flex-shrink-0 items-center justify-center overflow-hidden">
        <div className="relative flex h-full w-full flex-col items-center justify-center">
          <AnimatePresence mode="popLayout">
            <motion.div
              animate={{ y: 0, opacity: 1 }}
              className="flex flex-col items-center gap-2"
              exit={{ y: -20, opacity: 0 }}
              initial={{ y: 20, opacity: 0 }}
              key={currentIndex}
              transition={{ duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
            >
              {/* Previous phase (dimmed) */}
              {PrevIcon && (
                <PrevIcon
                  className="text-ink-faint opacity-30"
                  size={12}
                  weight="duotone"
                />
              )}

              {/* Current phase (highlighted) */}
              {CurrentIcon && (
                <CurrentIcon
                  className="text-ink-faint"
                  size={16}
                  weight="duotone"
                />
              )}

              {/* Next phase (dimmed) */}
              {NextIcon && (
                <NextIcon
                  className="text-ink-faint opacity-30"
                  size={12}
                  weight="duotone"
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
    <div className="flex h-full w-10 flex-shrink-0 items-center justify-center">
      <Icon className="text-ink-faint" size={16} weight="duotone" />
    </div>
  );
}
