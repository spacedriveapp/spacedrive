import SwiftUI

struct JobMonitorView: View {
    @EnvironmentObject var appState: SharedAppState

    var body: some View {
        VStack(spacing: 0) {
            // Header with connection status
            headerView

            Divider()
                .background(SpacedriveColors.Border.secondary)

            // Job list
            if appState.globalJobs.isEmpty {
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
                    .h5()

                HStack(spacing: 6) {
                    Circle()
                        .fill(connectionStatusColor)
                        .frame(width: 6, height: 6)

                    Text(appState.connectionStatus.displayName)
                        .labelSmall()
                }
            }

            Spacer()

            // Refresh button using new design system
            SDButton(
                "",
                style: .ghost,
                size: .small,
                icon: "arrow.clockwise"
            ) {
                appState.dispatch(.refreshJobs)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
    }

    private var connectionStatusColor: Color {
        switch appState.connectionStatus {
        case .connected:
            return SpacedriveColors.Accent.success
        case .connecting:
            return SpacedriveColors.Accent.warning
        case .disconnected, .error:
            return SpacedriveColors.Accent.error
        }
    }

    private var emptyStateView: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle")
                .font(.system(size: 48))
                .foregroundColor(SpacedriveColors.Accent.success.opacity(0.6))

            VStack(spacing: 8) {
                Text("No Active Jobs")
                    .h4()

                Text("All jobs are completed or no jobs are currently running.")
                    .bodySmall(color: SpacedriveColors.Text.secondary)
                    .multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(32)
    }

    private var jobListView: some View {
        ScrollView {
            LazyVStack(spacing: 4) {
                ForEach(appState.globalJobs) { job in
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
        .animation(.easeInOut(duration: 0.2), value: appState.globalJobs.count)
    }
}

#Preview {
    let appState = SharedAppState.shared

    // Add some sample jobs for preview
    appState.globalJobs = [
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

    return JobMonitorView()
        .environmentObject(appState)
        .frame(width: 400, height: 600)
        .background(SpacedriveColors.Background.primary)
}


