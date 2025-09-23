import SwiftUI
import AppKit

/// Job Companion Window - The floating job monitor window
struct JobCompanionView: View {
    @EnvironmentObject var appState: SharedAppState
    @Environment(\.windowType) var windowType

    var body: some View {
        ZStack {
            // Rounded background using design system colors
            RoundedBackgroundView(
                cornerRadius: 12,
                backgroundColor: SpacedriveColors.NSColors.backgroundPrimary
            )

            VStack(spacing: 0) {
                // Custom title bar with traffic lights integrated
                CustomTitleBar()

                // Main content - Job Monitor
                JobMonitorView()
                    .frame(minWidth: 300, minHeight: 400)
            }
        }
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
        )
    ]

    return JobCompanionView()
        .environmentObject(appState)
        .frame(width: 400, height: 600)
        .background(SpacedriveColors.Background.primary)
}
