import SwiftUI

struct JobRowView: View {
    let job: JobInfo

    private var progressPercentage: String {
        return String(format: "%.1f%%", job.progress * 100)
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
            return String(format: "%.1fs", duration)
        } else if duration < 3600 {
            return String(format: "%.1fm", duration / 60)
        } else {
            return String(format: "%.1fh", duration / 3600)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header with job name and status
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(job.name)
                        .font(.headline)
                        .foregroundColor(.primary)

                    Text("ID: \(String(job.id.prefix(8)))...")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }

                Spacer()

                HStack(spacing: 4) {
                    Text(job.status.icon)
                        .font(.title2)

                    Text(job.status.displayName)
                        .font(.caption)
                        .fontWeight(.medium)
                        .foregroundColor(statusColor)
                }
            }

            // Progress bar (only show for running jobs)
            if job.status == .running {
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Progress")
                            .font(.caption)
                            .foregroundColor(.secondary)

                        Spacer()

                        Text(progressPercentage)
                            .font(.caption)
                            .fontWeight(.medium)
                    }

                    ProgressView(value: job.progress, total: 1.0)
                        .progressViewStyle(LinearProgressViewStyle())
                        .scaleEffect(x: 1, y: 0.8, anchor: .center)
                }
            }

            // Error message (if any)
            if let errorMessage = job.errorMessage, !errorMessage.isEmpty {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundColor(.red)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.red.opacity(0.1))
                    .cornerRadius(4)
            }

            // Timestamps
            HStack {
                Text("Started \(timeAgo)")
                    .font(.caption)
                    .foregroundColor(.secondary)

                Spacer()

                if let duration = duration {
                    Text("Duration: \(duration)")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(backgroundColorForStatus)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(borderColorForStatus, lineWidth: 1)
        )
    }

    private var statusColor: Color {
        switch job.status {
        case .running:
            return .blue
        case .completed:
            return .green
        case .failed:
            return .red
        case .paused:
            return .orange
        case .queued:
            return .gray
        }
    }

    private var backgroundColorForStatus: Color {
        switch job.status {
        case .running:
            return Color.blue.opacity(0.05)
        case .completed:
            return Color.green.opacity(0.05)
        case .failed:
            return Color.red.opacity(0.05)
        case .paused:
            return Color.orange.opacity(0.05)
        case .queued:
            return Color.gray.opacity(0.05)
        }
    }

    private var borderColorForStatus: Color {
        switch job.status {
        case .running:
            return Color.blue.opacity(0.2)
        case .completed:
            return Color.green.opacity(0.2)
        case .failed:
            return Color.red.opacity(0.2)
        case .paused:
            return Color.orange.opacity(0.2)
        case .queued:
            return Color.gray.opacity(0.2)
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


