import SwiftUI

struct JobMonitorView: View {
    @ObservedObject var viewModel: JobListViewModel

    var body: some View {
        VStack(spacing: 0) {
            // Header with connection status
            headerView

            Divider()
                .background(Color.gray.opacity(0.3))

            // Job list
            if viewModel.jobs.isEmpty {
                emptyStateView
            } else {
                jobListView
            }
        }
        .background(Color.clear)
    }

    private var headerView: some View {
        HStack {
            VStack(alignment: .leading, spacing: 3) {
                Text("Spacedrive Jobs")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundColor(.primary)

                HStack(spacing: 6) {
                    Circle()
                        .fill(connectionStatusColor)
                        .frame(width: 6, height: 6)

                    Text(viewModel.connectionStatus.displayName)
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                }
            }

            Spacer()

            // Refresh button
            Button(action: {
                viewModel.reconnect()
            }) {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(.secondary)
            }
            .buttonStyle(PlainButtonStyle())
            .help("Reconnect to daemon")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
    }

    private var connectionStatusColor: Color {
        switch viewModel.connectionStatus {
        case .connected:
            return .green
        case .connecting:
            return .yellow
        case .disconnected, .error:
            return .red
        }
    }

    private var emptyStateView: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle")
                .font(.system(size: 48))
                .foregroundColor(.green.opacity(0.6))

            VStack(spacing: 8) {
                Text("No Active Jobs")
                    .font(.headline)
                    .foregroundColor(.primary)

                Text("All jobs are completed or no jobs are currently running.")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(32)
    }

    private var jobListView: some View {
        ScrollView {
            LazyVStack(spacing: 4) {
                ForEach(viewModel.jobs) { job in
                    JobRowView(job: job)
                        .transition(.asymmetric(
                            insertion: .scale.combined(with: .opacity),
                            removal: .opacity
                        ))
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
        }
        .animation(.easeInOut(duration: 0.2), value: viewModel.jobs.count)
    }
}

#Preview {
    let viewModel = JobListViewModel()

    // Add some sample jobs for preview
    viewModel.jobs = [
        JobInfo(
            id: "1",
            name: "file_indexer",
            status: .running,
            progress: 0.65,
            startedAt: Date().addingTimeInterval(-300),
            completedAt: nil,
            errorMessage: nil
        ),
        JobInfo(
            id: "2",
            name: "thumbnail_generator",
            status: .completed,
            progress: 1.0,
            startedAt: Date().addingTimeInterval(-600),
            completedAt: Date().addingTimeInterval(-60),
            errorMessage: nil
        ),
        JobInfo(
            id: "3",
            name: "file_copy",
            status: .failed,
            progress: 0.3,
            startedAt: Date().addingTimeInterval(-120),
            completedAt: Date().addingTimeInterval(-30),
            errorMessage: "Permission denied"
        )
    ]

    return JobMonitorView(viewModel: viewModel)
        .frame(width: 400, height: 600)
        .background(Color.black.opacity(0.1))
}


