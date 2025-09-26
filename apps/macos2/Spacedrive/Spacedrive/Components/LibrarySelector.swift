import SwiftUI
import SpacedriveClient

struct LibrarySelector: View {
    @EnvironmentObject var appState: SharedAppState
    @State private var showingLibraryPicker = false

    var body: some View {
        VStack(spacing: 12) {
            // Current library card
            if let currentLibrary = appState.currentLibrary {
                LibraryCard(library: currentLibrary, isSelected: true)
            } else if let currentLibraryId = appState.currentLibraryId,
                      let library = appState.availableLibraries.first(where: { $0.id == currentLibraryId })
            {
                LibraryCard(library: library, isSelected: true)
            } else {
                EmptyLibraryCard()
            }

            // Switch library button
            if appState.availableLibraries.count > 1 {
                Button(action: {
                    showingLibraryPicker = true
                }) {
                    HStack(spacing: 8) {
                        Image(systemName: "arrow.triangle.2.circlepath")
                            .font(.system(size: 13, weight: .medium))

                        Text("Switch Library")
                            .font(.system(size: 13, weight: .medium))
                    }
                    .foregroundColor(SpacedriveColors.Accent.primary)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(SpacedriveColors.Accent.primary.opacity(0.08))
                    )
                }
                .buttonStyle(PlainButtonStyle())
            }
        }
        .sheet(isPresented: $showingLibraryPicker) {
            LibraryPickerView()
        }
    }
}

/// Individual library card
struct LibraryCard: View {
    let library: LibraryInfo
    let isSelected: Bool

    var body: some View {
        VStack(spacing: 16) {
            // Top row: Icon, name, and path
            HStack(spacing: 12) {
                SpacedriveIconView(.database, size: 28)
                    .foregroundColor(SpacedriveColors.Accent.primary)

                VStack(alignment: .leading, spacing: 4) {
                    Text(library.name)
                        .font(.system(size: 18, weight: .semibold))
                        .foregroundColor(SpacedriveColors.Text.primary)

                    Text(library.path)
                        .font(.system(size: 12))
                        .foregroundColor(SpacedriveColors.Text.secondary)
                        .lineLimit(1)
                }

                Spacer()
            }

            // Bottom row: Statistics
            if let stats = library.stats {
                LibraryStatsView(stats: stats)
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 18)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(SpacedriveColors.Background.tertiary)
                .overlay(
                    RoundedRectangle(cornerRadius: 12)
                        .stroke(
                            isSelected ? SpacedriveColors.Border.primary : SpacedriveColors.Border.secondary,
                            lineWidth: isSelected ? 1 : 0.5
                        )
                )
        )
    }
}

/// Empty state card when no library is selected
struct EmptyLibraryCard: View {
    var body: some View {
        HStack(spacing: 12) {
            SpacedriveIconView(.database, size: 28)
                .foregroundColor(SpacedriveColors.Text.tertiary)

            VStack(alignment: .leading, spacing: 4) {
                Text("No Library Selected")
                    .font(.system(size: 18, weight: .medium))
                    .foregroundColor(SpacedriveColors.Text.secondary)

                Text("Select a library to view jobs")
                    .font(.system(size: 12))
                    .foregroundColor(SpacedriveColors.Text.tertiary)
            }

            Spacer()
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 18)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(SpacedriveColors.Background.tertiary)
                .overlay(
                    RoundedRectangle(cornerRadius: 12)
                        .stroke(SpacedriveColors.Border.secondary, lineWidth: 0.5)
                )
        )
    }
}

/// Library picker sheet
struct LibraryPickerView: View {
    @EnvironmentObject var appState: SharedAppState
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Select Library")
                    .font(.title2)
                    .fontWeight(.semibold)
                    .foregroundColor(SpacedriveColors.Text.primary)

                Spacer()

                Button("Done") {
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 16)
            .background(SpacedriveColors.Background.secondary)

            Divider()
                .background(SpacedriveColors.Border.primary)

            // Library list
            ScrollView {
                LazyVStack(spacing: 16) {
                    ForEach(appState.availableLibraries) { library in
                        LibraryCard(
                            library: library,
                            isSelected: library.id == appState.currentLibraryId
                        )
                        .onTapGesture {
                            appState.dispatch(.switchToLibrary(library))
                            dismiss()
                        }
                    }
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 20)
            }
            .background(SpacedriveColors.Background.primary)
        }
        .frame(width: 400, height: 500)
        .background(SpacedriveColors.Background.primary)
    }
}

/// Library statistics view with proper hierarchy
struct LibraryStatsView: View {
    let stats: LibraryStatistics

    var body: some View {
        HStack(spacing: 24) {
            // Hero: Total size (most prominent)
            VStack(alignment: .leading, spacing: 6) {
                Text(formatBytes(stats.totalSize))
                    .font(.system(size: 28, weight: .bold, design: .rounded))
                    .foregroundColor(SpacedriveColors.Text.primary)

                Text("Total Size")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundColor(SpacedriveColors.Text.secondary)
            }

            Spacer()

            // Secondary stats in a clean row
            HStack(spacing: 20) {
                StatItem(value: "\(stats.totalFiles)", label: "Files")
                StatItem(value: "\(stats.locationCount)", label: "Locations")
                StatItem(value: "\(stats.tagCount)", label: "Tags")
            }
        }
    }
}

/// Individual stat item with proper sizing
struct StatItem: View {
    let value: String
    let label: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(value)
                .font(.system(size: 16, weight: .semibold))
                .foregroundColor(SpacedriveColors.Text.primary)

            Text(label)
                .font(.system(size: 12))
                .foregroundColor(SpacedriveColors.Text.secondary)
        }
    }
}


/// Format bytes into human readable format
private func formatBytes(_ bytes: UInt64) -> String {
    let formatter = ByteCountFormatter()
    formatter.allowedUnits = [.useKB, .useMB, .useGB, .useTB]
    formatter.countStyle = .file
    return formatter.string(fromByteCount: Int64(bytes))
}

struct LibrarySelector_Previews: PreviewProvider {
    static var previews: some View {
        LibrarySelector()
            .environmentObject(SharedAppState.shared)
            .frame(width: 300)
    }
}
