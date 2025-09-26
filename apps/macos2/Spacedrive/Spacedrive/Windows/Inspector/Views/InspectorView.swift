import AppKit
import SpacedriveClient
import SwiftUI
import UniformTypeIdentifiers

/// Inspector window view for displaying file information
struct InspectorView: View {
    @StateObject private var viewModel = InspectorViewModel()

    var body: some View {
        VStack(spacing: 0) {
            // Header
            headerView

            Divider()

            // Content
            contentView
        }
        .background(Color(NSColor.windowBackgroundColor))
        .onDrop(of: [.fileURL], isTargeted: nil) { providers in
            handleFileDrop(providers)
        }
    }

    // MARK: - Header View

    private var headerView: some View {
        HStack {
            Image(systemName: "doc.text.magnifyingglass")
                .foregroundColor(.accentColor)
                .font(.title2)

            VStack(alignment: .leading, spacing: 2) {
                Text("File Inspector")
                    .font(.headline)
                    .foregroundColor(.primary)

                Text("Drag a file here to inspect")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Spacer()

            if viewModel.file != nil {
                Button("Clear") {
                    viewModel.clearFile()
                }
                .buttonStyle(.borderless)
            }
        }
        .padding()
    }

    // MARK: - Content View

    private var contentView: some View {
        Group {
            if viewModel.isLoading {
                loadingView
            } else if let errorMessage = viewModel.errorMessage {
                errorView(errorMessage)
            } else if let file = viewModel.file {
                fileInfoView(file)
            } else {
                emptyStateView
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Loading View

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.2)

            Text("Loading file information...")
                .font(.subheadline)
                .foregroundColor(.secondary)
        }
    }

    // MARK: - Error View

    private func errorView(_ message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.largeTitle)
                .foregroundColor(.red)

            Text("Error")
                .font(.headline)
                .foregroundColor(.primary)

            Text(message)
                .font(.subheadline)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
        }
    }

    // MARK: - Empty State View

    private var emptyStateView: some View {
        VStack(spacing: 20) {
            Image(systemName: "doc.badge.plus")
                .font(.system(size: 60))
                .foregroundColor(.secondary)

            VStack(spacing: 8) {
                Text("No File Selected")
                    .font(.headline)
                    .foregroundColor(.primary)

                Text("Drag and drop a file from your system to inspect its properties")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
            }
        }
    }

    // MARK: - File Info View

    private func fileInfoView(_ file: File) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // File Header
                fileHeaderView(file)

                Divider()

                // Basic Properties
                basicPropertiesView(file)

                Divider()

                // Content Identity
                if let contentIdentity = file.contentIdentity {
                    contentIdentityView(contentIdentity)

                    Divider()
                }

                // Tags
                if !file.tags.isEmpty {
                    tagsView(file.tags)

                    Divider()
                }

                // Sidecars
                if !file.sidecars.isEmpty {
                    sidecarsView(file.sidecars)

                    Divider()
                }

                // Alternate Paths
                if !file.alternatePaths.isEmpty {
                    alternatePathsView(file.alternatePaths)
                }
            }
            .padding()
        }
    }

    // MARK: - File Header

    private func fileHeaderView(_ file: File) -> some View {
        HStack(spacing: 12) {
            // File Icon
            Image(systemName: "doc")
                .font(.system(size: 40))
                .foregroundColor(.accentColor)

            VStack(alignment: .leading, spacing: 4) {
                Text(file.name)
                    .font(.headline)
                    .foregroundColor(.primary)

                Text("File Path")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(2)
            }

            Spacer()
        }
    }

    // MARK: - Basic Properties

    private func basicPropertiesView(_ file: File) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Properties")
                .font(.headline)
                .foregroundColor(.primary)

            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
            ], spacing: 8) {
                PropertyRow(label: "Size", value: ByteCountFormatter.string(fromByteCount: Int64(file.size), countStyle: .file))
                PropertyRow(label: "Created", value: file.createdAt)
                PropertyRow(label: "Modified", value: file.modifiedAt)
                PropertyRow(label: "Content Kind", value: file.contentKind.rawValue)
                PropertyRow(label: "Extension", value: file.extension ?? "None")
                PropertyRow(label: "Is Local", value: file.isLocal ? "Yes" : "No")
            }
        }
    }

    // MARK: - Content Identity

    private func contentIdentityView(_ contentIdentity: ContentIdentity) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Content Identity")
                .font(.headline)
                .foregroundColor(.primary)

            VStack(alignment: .leading, spacing: 4) {
                PropertyRow(label: "UUID", value: contentIdentity.uuid)
                PropertyRow(label: "Kind", value: contentIdentity.kind.rawValue)
                PropertyRow(label: "Hash", value: String(contentIdentity.hash.prefix(16)) + "...")
                PropertyRow(label: "Created", value: contentIdentity.createdAt)
            }
        }
    }

    // MARK: - Tags

    private func tagsView(_ tags: [Tag]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Tags")
                .font(.headline)
                .foregroundColor(.primary)

            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
            ], spacing: 8) {
                ForEach(tags, id: \.id) { tag in
                    TagChip(tag: tag)
                }
            }
        }
    }

    // MARK: - Sidecars

    private func sidecarsView(_ sidecars: [Sidecar]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Sidecars")
                .font(.headline)
                .foregroundColor(.primary)

            ForEach(sidecars, id: \.id) { sidecar in
                SidecarRow(sidecar: sidecar)
            }
        }
    }

    // MARK: - Alternate Paths

    private func alternatePathsView(_ paths: [SdPath]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Alternate Paths")
                .font(.headline)
                .foregroundColor(.primary)

            ForEach(paths.indices, id: \.self) { _ in
                HStack {
                    Image(systemName: "doc.on.doc")
                        .foregroundColor(.secondary)

                    Text("Alternate Path")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    Spacer()
                }
                .padding(.vertical, 2)
            }
        }
    }

    // MARK: - File Drop Handler

    private func handleFileDrop(_ providers: [NSItemProvider]) -> Bool {
        guard let provider = providers.first else { return false }

        provider.loadItem(forTypeIdentifier: "public.file-url", options: nil) { item, _ in
            guard let data = item as? Data,
                  let url = URL(dataRepresentation: data, relativeTo: nil)
            else {
                return
            }

            DispatchQueue.main.async {
                if viewModel.isValidFileURL(url) {
                    viewModel.loadFileByPath(url)
                }
            }
        }

        return true
    }
}

// MARK: - Helper Views

struct PropertyRow: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.caption)
                .foregroundColor(.secondary)

            Text(value)
                .font(.subheadline)
                .foregroundColor(.primary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct TagChip: View {
    let tag: Tag

    var body: some View {
        HStack {
            Circle()
                .fill(Color(hex: tag.color ?? "#666666"))
                .frame(width: 8, height: 8)

            Text(tag.displayName ?? tag.canonicalName)
                .font(.caption)
                .foregroundColor(.primary)

            Spacer()
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Color(NSColor.controlBackgroundColor))
        .cornerRadius(4)
    }
}

struct SidecarRow: View {
    let sidecar: Sidecar

    var body: some View {
        HStack {
            Image(systemName: "doc.badge.gearshape")
                .foregroundColor(.secondary)

            VStack(alignment: .leading, spacing: 2) {
                Text(sidecar.kind)
                    .font(.subheadline)
                    .foregroundColor(.primary)

                Text("\(ByteCountFormatter.string(fromByteCount: sidecar.size, countStyle: .file)) • \(sidecar.status)")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Spacer()

            Text(sidecar.format)
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding(.vertical, 4)
    }
}

// MARK: - Extensions

extension DateFormatter {
    static let shortDateTime: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .short
        formatter.timeStyle = .short
        return formatter
    }()
}

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let a, r, g, b: UInt64
        switch hex.count {
        case 3: // RGB (12-bit)
            (a, r, g, b) = (255, (int >> 8) * 17, (int >> 4 & 0xF) * 17, (int & 0xF) * 17)
        case 6: // RGB (24-bit)
            (a, r, g, b) = (255, int >> 16, int >> 8 & 0xFF, int & 0xFF)
        case 8: // ARGB (32-bit)
            (a, r, g, b) = (int >> 24, int >> 16 & 0xFF, int >> 8 & 0xFF, int & 0xFF)
        default:
            (a, r, g, b) = (1, 1, 1, 0)
        }

        self.init(
            .sRGB,
            red: Double(r) / 255,
            green: Double(g) / 255,
            blue: Double(b) / 255,
            opacity: Double(a) / 255
        )
    }
}
