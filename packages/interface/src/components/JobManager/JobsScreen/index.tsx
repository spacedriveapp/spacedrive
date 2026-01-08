import { FunnelSimple, X } from "@phosphor-icons/react";
import { TopBarButton } from "@sd/ui";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useJobs } from "../hooks/useJobs";
import { JobRow } from "./JobRow";

export function JobsScreen() {
  const navigate = useNavigate();
  const { jobs, pause, resume, cancel } = useJobs();
  const [showOnlyRunning, setShowOnlyRunning] = useState(false);

  // Filter jobs based on toggle
  const filteredJobs = showOnlyRunning
    ? jobs.filter((job) => job.status === "running" || job.status === "paused")
    : jobs;

  // Group jobs by status
  const runningJobs = filteredJobs.filter((j) => j.status === "running");
  const pausedJobs = filteredJobs.filter((j) => j.status === "paused");
  const queuedJobs = filteredJobs.filter((j) => j.status === "queued");
  const completedJobs = filteredJobs.filter((j) => j.status === "completed");
  const failedJobs = filteredJobs.filter((j) => j.status === "failed");

  return (
    <div className="flex h-screen flex-col bg-app">
      {/* Header */}
      <div className="sticky top-0 z-10 border-app-line border-b bg-app/80 backdrop-blur-xl">
        <div className="flex items-center justify-between px-6 py-4">
          <div className="flex items-center gap-4">
            <h1 className="font-bold text-2xl text-ink">Jobs</h1>
            <div className="flex items-center gap-2 text-ink-dull text-sm">
              <span>{jobs.length} total</span>
              {runningJobs.length > 0 && (
                <>
                  <span>•</span>
                  <span>{runningJobs.length} running</span>
                </>
              )}
            </div>
          </div>

          <div className="flex items-center gap-2">
            {/* Filter toggle */}
            <TopBarButton
              active={showOnlyRunning}
              icon={FunnelSimple}
              onClick={() => setShowOnlyRunning(!showOnlyRunning)}
              title={
                showOnlyRunning ? "Show all jobs" : "Show only active jobs"
              }
            />

            {/* Back button */}
            <TopBarButton
              icon={X}
              onClick={() => navigate(-1)}
              title="Go back"
            />
          </div>
        </div>

        {/* Column headers */}
        <div className="flex items-center gap-4 border-app-line/30 border-t bg-app-box/30 px-4 py-2 font-medium text-ink-dull text-xs uppercase tracking-wide">
          <div className="w-10 flex-shrink-0" /> {/* Icon spacer */}
          <div className="flex min-w-0 flex-1 items-center gap-6">
            <div className="flex-1">Name</div>
            <div className="w-32 flex-shrink-0">Duration</div>
            <div className="w-24 flex-shrink-0 text-right">Time</div>
            <div className="w-20 flex-shrink-0 text-right">Status</div>
          </div>
          <div className="w-6 flex-shrink-0" /> {/* Action button spacer */}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {filteredJobs.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <div className="text-center">
              <p className="text-ink-dull text-sm">No jobs found</p>
            </div>
          </div>
        ) : (
          <div>
            {/* Running Jobs */}
            {runningJobs.length > 0 && (
              <JobSection count={runningJobs.length} title="Running">
                {runningJobs.map((job) => (
                  <JobRow
                    job={job}
                    key={job.id}
                    onCancel={cancel}
                    onPause={pause}
                    onResume={resume}
                  />
                ))}
              </JobSection>
            )}

            {/* Paused Jobs */}
            {pausedJobs.length > 0 && (
              <JobSection count={pausedJobs.length} title="Paused">
                {pausedJobs.map((job) => (
                  <JobRow
                    job={job}
                    key={job.id}
                    onCancel={cancel}
                    onPause={pause}
                    onResume={resume}
                  />
                ))}
              </JobSection>
            )}

            {/* Queued Jobs */}
            {queuedJobs.length > 0 && (
              <JobSection count={queuedJobs.length} title="Queued">
                {queuedJobs.map((job) => (
                  <JobRow
                    job={job}
                    key={job.id}
                    onCancel={cancel}
                    onPause={pause}
                    onResume={resume}
                  />
                ))}
              </JobSection>
            )}

            {/* Completed Jobs */}
            {completedJobs.length > 0 && (
              <JobSection count={completedJobs.length} title="Completed">
                {completedJobs.map((job) => (
                  <JobRow
                    job={job}
                    key={job.id}
                    onCancel={cancel}
                    onPause={pause}
                    onResume={resume}
                  />
                ))}
              </JobSection>
            )}

            {/* Failed Jobs */}
            {failedJobs.length > 0 && (
              <JobSection count={failedJobs.length} title="Failed">
                {failedJobs.map((job) => (
                  <JobRow
                    job={job}
                    key={job.id}
                    onCancel={cancel}
                    onPause={pause}
                    onResume={resume}
                  />
                ))}
              </JobSection>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

interface JobSectionProps {
  title: string;
  count: number;
  children: React.ReactNode;
}

function JobSection({ title, count, children }: JobSectionProps) {
  return (
    <div>
      <div className="sticky top-0 z-10 flex items-center gap-2 border-app-line/50 border-b bg-app-box/50 px-4 py-2 backdrop-blur-sm">
        <h2 className="font-semibold text-ink text-xs uppercase tracking-wide">
          {title}
        </h2>
        <span className="text-ink-dull text-xs">({count})</span>
      </div>
      <div>{children}</div>
    </div>
  );
}
