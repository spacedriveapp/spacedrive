import SwiftUI

struct JobRowView: View {
    let job: JobInfo

    private var progressPercentage: String {
        return String(format: "%.0f%%", job.progress * 100)
    }

    private var timeAgo: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: job.startedAt, relativeTo: Date())
    }

    private var duration: String? {
        guard let completedAt = job.completedAt else { return nil }
        let duration = completedAt.timeIntervalSince(job.startedAt)

        if duration < 60 {
            return String(format: "%.0fs", duration)
        } else if duration < 3600 {
            return String(format: "%.0fm", duration / 60)
        } else {
            return String(format: "%.1fh", duration / 3600)
        }
    }

    private var subtextForJob: String {
        // Show current path when available and job is running
        if job.status == .running, let currentPath = job.currentPath, !currentPath.isEmpty {
            return currentPath
        }

        // Default subtext based on job status and type
        switch job.status {
        case .running:
            if let phase = job.currentPhase {
                return phase
            } else {
                return "Processing..."
            }
        case .completed:
            if let duration = duration {
                return "Completed in \(duration)"
            } else {
                return "Completed successfully"
            }
        case .failed:
            return job.errorMessage ?? "Job failed"
        case .paused:
            return "Paused"
        case .queued:
            return "Waiting to start"
        }
    }

    var body: some View {
        SDJobCard {
            HStack(spacing: 12) {
            // Status indicator - simple colored circle
            Circle()
                .fill(statusColor)
                .frame(width: 8, height: 8)
                .opacity(job.status == .completed ? 1.0 : 0.8)

            // Main content area
            VStack(alignment: .leading, spacing: 2) {
                // Top row: Job name and status/progress
                HStack {
                    VStack(alignment: .leading, spacing: 1) {
                        // Job title
                        Text(job.name)
                            .font(.system(size: 13, weight: .medium))
                            .foregroundColor(.primary)
                            .lineLimit(1)
                            .truncationMode(.tail)

                        // Subtext: Current path or default text to maintain consistent layout
                        Text(subtextForJob)
                            .font(.system(size: 10))
                            .foregroundColor(.secondary)
                            .opacity(0.7)
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }

                    Spacer()

                    // Status and progress info
                    HStack(spacing: 6) {
                        if job.status == .running {
                            // Show phase if available, otherwise percentage
                            if let phase = job.currentPhase, !phase.isEmpty {
                                Text(phase)
                                    .font(.system(size: 11, weight: .medium))
                                    .foregroundColor(.secondary)
                            } else {
                                Text(progressPercentage)
                                    .font(.system(size: 11, weight: .medium))
                                    .foregroundColor(.secondary)
                            }
                        } else {
                            Text(job.status.displayName)
                                .font(.system(size: 11, weight: .medium))
                                .foregroundColor(job.status == .failed ? .red : .secondary)
                        }

                        // Show completion info or time info
                        if let completionInfo = job.completionInfo, !completionInfo.isEmpty, job.status == .running {
                            Text("• \(completionInfo)")
                                .font(.system(size: 11))
                                .foregroundColor(.secondary)
                                .opacity(0.7)
                        } else if let duration = duration {
                            Text("• \(duration)")
                                .font(.system(size: 11))
                                .foregroundColor(.secondary)
                                .opacity(0.7)
                        } else if job.status == .running {
                            Text("• \(timeAgo)")
                                .font(.system(size: 11))
                                .foregroundColor(.secondary)
                                .opacity(0.7)
                        }

                        // Show issues indicator if present
                        if job.hasIssues, let issuesInfo = job.issuesInfo {
                            Text("⚠️")
                                .font(.system(size: 10))
                                .foregroundColor(.orange)
                                .help(issuesInfo)
                        }
                    }
                }

                // Progress bar for active jobs
                if job.status == .running || job.status == .paused {
                    ProgressView(value: job.progress, total: 1.0)
                        .progressViewStyle(LinearProgressViewStyle(tint: statusColor))
                        .scaleEffect(x: 1, y: 0.6, anchor: .center)
                } else if job.status == .completed {
                    // Completed indicator line
                    Rectangle()
                        .fill(Color.green.opacity(0.3))
                        .frame(height: 2)
                        .cornerRadius(1)
                }

                // Error message (compact display)
                if let errorMessage = job.errorMessage, !errorMessage.isEmpty {
                    Text(errorMessage)
                        .font(.system(size: 11))
                        .foregroundColor(.red)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
            }
            }
        }
    }

    private var statusColor: Color {
        switch job.status {
        case .running:
            return SpacedriveColors.Accent.info
        case .completed:
            return SpacedriveColors.Accent.success
        case .failed:
            return SpacedriveColors.Accent.error
        case .paused:
            return SpacedriveColors.Accent.warning
        case .queued:
            return SpacedriveColors.Text.tertiary
        }
    }
}

#Preview {
    VStack(spacing: 12) {
        JobRowView(job: JobInfo(
            id: "12345678-1234-1234-1234-123456789012",
            name: "file_indexer",
            status: .running,
            progress: 0.65,
            startedAt: Date().addingTimeInterval(-300),
            completedAt: nil,
            errorMessage: nil
        ))

        JobRowView(job: JobInfo(
            id: "87654321-4321-4321-4321-210987654321",
            name: "file_copy",
            status: .completed,
            progress: 1.0,
            startedAt: Date().addingTimeInterval(-600),
            completedAt: Date().addingTimeInterval(-60),
            errorMessage: nil
        ))

        JobRowView(job: JobInfo(
            id: "11111111-2222-3333-4444-555555555555",
            name: "thumbnail_generator",
            status: .failed,
            progress: 0.3,
            startedAt: Date().addingTimeInterval(-120),
            completedAt: Date().addingTimeInterval(-30),
            errorMessage: "Failed to process image: unsupported format"
        ))
    }
    .padding()
    .background(Color.black.opacity(0.1))
}


