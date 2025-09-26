import SwiftUI

struct BrowserView: View {
    @StateObject private var browserState = BrowserState()
    @State private var searchText = ""
    @State private var showingInspector = true
    @State private var selectedItem: BrowserItem?

    var body: some View {
        NavigationSplitView {
            // Sidebar
            SidebarView(browserState: browserState)
                .navigationSplitViewColumnWidth(min: 200, ideal: 250)
        } content: {
            // Main content
            ContentView(browserState: browserState, selectedItem: $selectedItem)
        } detail: {
            // Inspector
            if showingInspector {
                InspectorDetailView(selectedItem: selectedItem)
                    .navigationSplitViewColumnWidth(min: 200, ideal: 220)
            }
        }
        .searchable(text: $searchText, prompt: "Search files...")
        .toolbar {
            // View controls
            ToolbarItemGroup(placement: .primaryAction) {
                Button(action: { browserState.toggleSidebar() }) {
                    Image(systemName: "sidebar.left")
                }
                .buttonStyle(.glass)

                Button(action: { showingInspector.toggle() }) {
                    Image(systemName: "sidebar.right")
                }
                .buttonStyle(.glass)
            }

            // Action buttons
            ToolbarItemGroup(placement: .secondaryAction) {
                Button("New Folder") {
                    // TODO: Implement new folder
                }
                .buttonStyle(.glass)

                Button("Import") {
                    // TODO: Implement import
                }
                .buttonStyle(.glass)
            }
        }
        .backgroundExtensionEffect() // Enable content to extend behind translucent elements
        .onChange(of: selectedItem) { _, newValue in
            // Handle selection changes
        }
    }
}

// MARK: - Sidebar View
struct SidebarView: View {
    @ObservedObject var browserState: BrowserState

    var body: some View {
        List(selection: $browserState.selectedLocation) {
            locationsSection
            recentSection
        }
        .listStyle(.sidebar)
        .navigationTitle("Spacedrive")
    }

    private var locationsSection: some View {
        Section("Locations") {
            ForEach(browserState.locations) { location in
                FinderSidebarLocationRow(
                    location: location,
                    isSelected: browserState.selectedLocation?.id == location.id
                ) {
                    browserState.selectLocation(location)
                }
            }
        }
    }

    private var recentSection: some View {
        Section("Recent") {
            ForEach(browserState.recentItems.prefix(5)) { item in
                Button(action: {
                    // TODO: Handle recent item selection
                }) {
                    HStack(spacing: 8) {
                        Image(systemName: item.icon)
                            .font(.system(size: 16))
                            .foregroundColor(.secondary)
                            .frame(width: 20)

                        VStack(alignment: .leading, spacing: 2) {
                            Text(item.name)
                                .font(.system(size: 13))
                                .foregroundColor(.primary)
                                .lineLimit(1)

                            Text(item.modifiedDate)
                                .font(.system(size: 11))
                                .foregroundColor(.secondary)
                        }

                        Spacer()
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                }
                .buttonStyle(PlainButtonStyle())
            }
        }
    }
}

// MARK: - Content View
struct ContentView: View {
    @ObservedObject var browserState: BrowserState
    @Binding var selectedItem: BrowserItem?

    var body: some View {
        VStack(spacing: 0) {
            // Breadcrumb navigation
            HStack {
                Button(action: {}) {
                    HStack {
                        Image(systemName: "chevron.left")
                        Text("Back")
                    }
                }
                .buttonStyle(.glass)

                Spacer()

                Text(browserState.currentPath)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .padding()

            Divider()

            // Content grid
            ScrollView {
                LazyVGrid(columns: Array(repeating: GridItem(.adaptive(minimum: 120)), count: 1)) {
                    ForEach(browserState.currentItems) { item in
                        ContentItemCard(item: item) {
                            selectedItem = item
                        }
                    }
                }
                .padding()
            }
        }
        .navigationTitle(browserState.selectedLocation?.name ?? "Browser")
    }
}

// MARK: - Content Item Card
struct ContentItemCard: View {
    let item: BrowserItem
    let onTap: () -> Void

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: item.icon)
                .font(.system(size: 32))
                .foregroundColor(.primary)

            Text(item.name)
                .font(.caption)
                .lineLimit(2)
                .multilineTextAlignment(.center)

            Text(item.size)
                .font(.caption2)
                .foregroundColor(.secondary)
        }
        .frame(width: 100, height: 100)
        .background(.ultraThinMaterial)
        .clipShape(UnevenRoundedRectangle(
            topLeadingRadius: 9,
            bottomLeadingRadius: 9,
            bottomTrailingRadius: 9,
            topTrailingRadius: 9
        ))
        .onTapGesture {
            onTap()
        }
    }
}

// MARK: - Inspector Detail View
struct InspectorDetailView: View {
    let selectedItem: BrowserItem?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let item = selectedItem {
                // File preview
                VStack {
                    Image(systemName: item.icon)
                        .font(.system(size: 48))
                        .foregroundColor(.primary)

                    Text(item.name)
                        .font(.subheadline)
                        .multilineTextAlignment(.center)
                        .lineLimit(2)
                }
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(.ultraThinMaterial)
                .clipShape(UnevenRoundedRectangle(
                    topLeadingRadius: 9,
                    bottomLeadingRadius: 9,
                    bottomTrailingRadius: 9,
                    topTrailingRadius: 9
                ))

                // File details
                VStack(alignment: .leading, spacing: 6) {
                    Text("Details")
                        .font(.subheadline)
                        .fontWeight(.semibold)

                    BrowserPropertyRow(label: "Type", value: item.type)
                    BrowserPropertyRow(label: "Size", value: item.size)
                    BrowserPropertyRow(label: "Modified", value: item.modifiedDate)
                    BrowserPropertyRow(label: "Path", value: item.path)
                }

                Spacer()
            } else {
                VStack {
                    Image(systemName: "doc")
                        .font(.system(size: 32))
                        .foregroundColor(.secondary)

                    Text("No Selection")
                        .font(.subheadline)
                        .foregroundColor(.secondary)

                    Text("Select a file to view its details")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .navigationTitle("Inspector")
    }
}

// MARK: - Property Row
struct BrowserPropertyRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack {
            Text(label)
                .font(.caption)
                .foregroundColor(.secondary)
            Spacer()
            Text(value)
                .font(.caption)
                .textSelection(.enabled)
        }
    }
}

// MARK: - Finder-Style Sidebar Location Row
struct FinderSidebarLocationRow: View {
    let location: BrowserLocation
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: location.iconName)
                    .font(.system(size: 16))
                    .foregroundColor(.primary)
                    .frame(width: 20)

                Text(location.name)
                    .font(.system(size: 13))
                    .foregroundColor(.primary)
                    .lineLimit(1)

                Spacer()
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(isSelected ? Color.accentColor.opacity(0.15) : Color.clear)
            )
        }
        .buttonStyle(PlainButtonStyle())
    }
}

#Preview {
    BrowserView()
        .frame(width: 1200, height: 800)
}
