import SpacedriveClient
import SwiftUI

struct JobRowView: View {
    let job: JobInfo
    @Environment(\.window) private var window
    @State private var isWindowActive = true

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

        // Show context info if available (for queued jobs especially)
        if let contextInfo = job.contextInfo, !contextInfo.isEmpty {
            return contextInfo
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
        case .cancelled:
            return "Cancelled"
        }
    }

    var body: some View {
        SDJobCard {
            HStack(spacing: 10) {
                // Status indicator - simple colored circle
                Circle()
                    .fill(statusColor)
                    .frame(width: 8, height: 8)
                    .opacity(job.status == .completed ? 1.0 : 0.8)

                // Main content area with equal spacing between rows
                VStack(alignment: .leading, spacing: 6) {
                    // Row 1: Job title and status
                    HStack {
                        Text(job.displayName)
                            .font(.system(size: 13, weight: .medium))
                            .foregroundColor(SpacedriveColors.Text.primary)
                            .lineLimit(1)
                            .padding(.bottom, -2)
                            .truncationMode(.tail)

                        Spacer()
                        // Status and progress info
                        HStack(spacing: 6) {
                            // Pause/Resume button for active jobs
                            if job.status == .running || job.status == .paused {
                                JobActionButton(job: job)
                            }
                            if job.status == .running {
                                // Show phase if available, otherwise percentage
                                if let phase = job.currentPhase, !phase.isEmpty {
                                    Text(phase)
                                        .font(.system(size: 11, weight: .medium))
                                        .foregroundColor(SpacedriveColors.Text.secondary)
                                } else {
                                    Text(progressPercentage)
                                        .font(.system(size: 11, weight: .medium))
                                        .foregroundColor(SpacedriveColors.Text.secondary)
                                }
                            } else {
                                Text(job.status.displayName)
                                    .font(.system(size: 11, weight: .medium))
                                    .foregroundColor(job.status == .failed ? SpacedriveColors.Accent.error : SpacedriveColors.Text.secondary)
                            }

                            // Show completion info or time info
                            if let completionInfo = job.completionInfo, !completionInfo.isEmpty, job.status == .running {
                                Text("• \(completionInfo)")
                                    .font(.system(size: 11))
                                    .foregroundColor(SpacedriveColors.Text.secondary)
                                    .opacity(0.7)
                            } else if let duration = duration {
                                Text("• \(duration)")
                                    .font(.system(size: 11))
                                    .foregroundColor(SpacedriveColors.Text.secondary)
                                    .opacity(0.7)
                            } else if job.status == .running {
                                Text("• \(timeAgo)")
                                    .font(.system(size: 11))
                                    .foregroundColor(SpacedriveColors.Text.secondary)
                                    .opacity(0.7)
                            }

                            // Show issues indicator if present
                            if job.hasIssues, let issuesInfo = job.issuesInfo {
                                Text("⚠️")
                                    .font(.system(size: 10))
                                    .foregroundColor(SpacedriveColors.Accent.warning)
                                    .help(issuesInfo)
                            }
                        }
                    }

                    // Row 2: Subtext
                    Text(subtextForJob)
                        .font(.system(size: 10))
                        .foregroundColor(SpacedriveColors.Text.secondary)
                        .opacity(0.7)
                        .padding(.bottom, 2)
                        .lineLimit(1)
                        .truncationMode(.tail)

                    // Row 3: Progress bar
                    HStack {
                        if job.status == .running || job.status == .paused {
                            ProgressView(value: job.progress, total: 1.0)
                                .progressViewStyle(LinearProgressViewStyle(tint: statusColor))
                                .frame(height: 4)
                        } else if job.status == .completed {
                            // Completed indicator line
                             ProgressView(value: job.progress, total: 1.0)
                                .progressViewStyle(LinearProgressViewStyle(tint: SpacedriveColors.Accent.success.opacity(0.6)))
                                .frame(height: 4)
                        } else {
                            // Empty space for other statuses to maintain consistent height
                            Rectangle()
                                .fill(Color.clear)
                                .frame(height: 4)
                        }

                        Spacer()
                    }

                    // Error message (if present)
                    if let errorMessage = job.errorMessage, !errorMessage.isEmpty {
                        Text(errorMessage)
                            .font(.system(size: 11))
                            .foregroundColor(SpacedriveColors.Accent.error)
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }
                }
                .frame(height: 52) // Reduced total height with better spacing
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { _ in
            updateWindowActiveState()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification)) { _ in
            updateWindowActiveState()
        }
        .onAppear {
            updateWindowActiveState()
        }
    }

    private func updateWindowActiveState() {
        isWindowActive = window?.isKeyWindow ?? false
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
        case .cancelled:
            return SpacedriveColors.Accent.error
        }
    }
}

/// Small action button for pause/resume job functionality
struct JobActionButton: View {
    let job: JobInfo
    @EnvironmentObject var appState: SharedAppState
    @State private var isHovered = false

    var body: some View {
        Button(action: {
            performAction()
        }) {
            Image(systemName: iconName)
                .font(.system(size: 10, weight: .medium))
                .foregroundColor(iconColor)
                .frame(width: 16, height: 16)
                .background(
                    Circle()
                        .fill(backgroundColor)
                        .frame(width: 16, height: 16)
                )
        }
        .buttonStyle(PlainButtonStyle())
        .help(helpText)
        .onHover { hovering in
            isHovered = hovering
        }
    }

    private var iconName: String {
        switch job.status {
        case .running:
            return "pause.fill"
        case .paused:
            return "play.fill"
        default:
            return "pause.fill"
        }
    }

    private var iconColor: Color {
        if isHovered {
            return SpacedriveColors.Text.primary
        } else {
            return SpacedriveColors.Text.secondary
        }
    }

    private var backgroundColor: Color {
        if isHovered {
            return SpacedriveColors.Interactive.hover
        } else {
            return Color.clear
        }
    }

    private var helpText: String {
        switch job.status {
        case .running:
            return "Pause job"
        case .paused:
            return "Resume job"
        default:
            return "Job action"
        }
    }

    private func performAction() {
        switch job.status {
        case .running:
            appState.dispatch(.pauseJob(job.id))
        case .paused:
            appState.dispatch(.resumeJob(job.id))
        default:
            break
        }
    }
}

#Preview {
    VStack(spacing: 12) {
        JobRowView(job: JobInfo(
            id: "12345678-1234-1234-1234-123456789012",
            name: "indexer",
            status: .running,
            progress: 0.65,
            startedAt: Date().addingTimeInterval(-300),
            completedAt: nil,
            errorMessage: nil,
            actionType: "locations.add",
            actionContext: ActionContextInfo(
                actionType: "locations.add",
                initiatedAt: "2025-01-24T20:24:00.820790Z",
                initiatedBy: "user",
                actionInput: JsonValue.objectValue([
                    "path": JsonValue.stringValue("/Users/jamespine/Downloads"),
                    "name": JsonValue.stringValue("Downloads"),
                ]),
                context: JsonValue.objectValue([
                    "operation": JsonValue.stringValue("add_location"),
                    "trigger": JsonValue.stringValue("user_action"),
                ])
            )
        ))

        JobRowView(job: JobInfo(
            id: "87654321-4321-4321-4321-210987654321",
            name: "indexer",
            status: .completed,
            progress: 1.0,
            startedAt: Date().addingTimeInterval(-600),
            completedAt: Date().addingTimeInterval(-60),
            errorMessage: nil,
            actionType: "files.copy",
            actionContext: ActionContextInfo(
                actionType: "files.copy",
                initiatedAt: "2025-01-24T19:30:00.820790Z",
                initiatedBy: "user",
                actionInput: JsonValue.objectValue([
                    "source": JsonValue.stringValue("/Users/jamespine/Documents"),
                    "destination": JsonValue.stringValue("/Users/jamespine/Backup"),
                ]),
                context: JsonValue.objectValue([
                    "operation": JsonValue.stringValue("copy_files"),
                    "trigger": JsonValue.stringValue("user_action"),
                ])
            )
        ))

        JobRowView(job: JobInfo(
            id: "11111111-2222-3333-4444-555555555555",
            name: "thumbnail_generator",
            status: .failed,
            progress: 0.3,
            startedAt: Date().addingTimeInterval(-120),
            completedAt: Date().addingTimeInterval(-30),
            errorMessage: "Failed to process image: unsupported format",
            actionType: "media.thumbnail",
            actionContext: ActionContextInfo(
                actionType: "media.thumbnail",
                initiatedAt: "2025-01-24T20:00:00.820790Z",
                initiatedBy: "system",
                actionInput: JsonValue.objectValue([
                    "file_count": JsonValue.stringValue("25"),
                    "file_types": JsonValue.arrayValue([JsonValue.stringValue("jpg"), JsonValue.stringValue("png")]),
                ]),
                context: JsonValue.objectValue([
                    "operation": JsonValue.stringValue("generate_thumbnails"),
                    "trigger": JsonValue.stringValue("system"),
                ])
            )
        ))
    }
    .padding()
    .background(Color.black.opacity(0.1))
}
