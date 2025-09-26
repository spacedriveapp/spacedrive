import SwiftUI

/// Comprehensive icon showcase window for Spacedrive icons
struct IconShowcaseView: View {
    @State private var searchText = ""
    @State private var selectedCategory: IconCategory = .all
    @State private var selectedSize: IconSize = .default
    @State private var showLightVariants = false
    @State private var show20pxVariants = false
    @EnvironmentObject var appState: SharedAppState

    enum IconCategory: String, CaseIterable {
        case all = "All Icons"
        case fileTypes = "File Types"
        case drives = "Drives & Cloud"
        case devices = "Devices"
        case ui = "UI Elements"

        var icons: [SpacedriveIcon] {
            switch self {
            case .all:
                return SpacedriveIcon.allCases
            case .fileTypes:
                return SpacedriveIcon.allCases.filter { icon in
                    ["Album", "Alias", "Application", "Archive", "Audio", "Book", "Collection",
                     "Database", "Document", "Encrypted", "Entity", "Executable", "Folder",
                     "Game", "Image", "Key", "Link", "Lock", "Mesh", "Movie", "Package",
                     "Screenshot", "Text", "TexturedMesh", "Trash", "Undefined", "Video", "Widget"]
                        .contains(icon.baseName)
                }
            case .drives:
                return SpacedriveIcon.allCases.filter { icon in
                    icon.rawValue.contains("Drive") ||
                        ["AmazonS3", "BackBlaze", "Box", "DAV", "Dropbox", "GoogleDrive", "Mega",
                         "OneDrive", "OpenStack", "PCloud", "Location", "Sync", "Spacedrop"]
                        .contains(icon.baseName)
                }
            case .devices:
                return SpacedriveIcon.allCases.filter { icon in
                    ["Ball", "Globe", "HDD", "Heart", "Home", "Laptop", "Mobile", "PC",
                     "Server", "Tablet", "Terminal"]
                        .contains(icon.baseName)
                }
            case .ui:
                return SpacedriveIcon.allCases.filter { icon in
                    ["Search", "Tags", "Scrapbook", "Screenshot", "Face", "Entity"]
                        .contains(icon.baseName)
                }
            }
        }
    }

    enum IconSize: String, CaseIterable {
        case small = "Small (16px)"
        case medium = "Medium (24px)"
        case large = "Large (32px)"
        case xlarge = "Extra Large (48px)"

        var size: CGFloat {
            switch self {
            case .small: return 16
            case .medium: return 24
            case .large: return 32
            case .xlarge: return 48
            }
        }

        static let `default` = IconSize.xlarge
    }

    private var filteredIcons: [SpacedriveIcon] {
        let categoryIcons = selectedCategory.icons

        let filtered = categoryIcons.filter { icon in
            // Search filter
            let matchesSearch = searchText.isEmpty ||
                icon.displayName.localizedCaseInsensitiveContains(searchText) ||
                icon.rawValue.localizedCaseInsensitiveContains(searchText)

            // Variant filters - hide variants by default unless explicitly shown
            let matchesLightFilter = showLightVariants || !icon.isLightVariant
            let matches20pxFilter = show20pxVariants || !icon.is20pxVariant

            return matchesSearch && matchesLightFilter && matches20pxFilter
        }

        return filtered.sorted { $0.displayName < $1.displayName }
    }

    var body: some View {
        WindowContainer {
            NavigationView {
                // Sidebar
                VStack(alignment: .leading, spacing: 16) {
                    Text("Icon Showcase")
                        .font(.title2)
                        .fontWeight(.bold)
                        .foregroundColor(SpacedriveColors.Text.primary)
                        .padding(.horizontal)
                        .padding(.top, 16)

                    // Search
                    HStack {
                        Image(systemName: "magnifyingglass")
                            .foregroundColor(SpacedriveColors.Text.secondary)
                        TextField("Search icons...", text: $searchText)
                            .textFieldStyle(RoundedBorderTextFieldStyle())
                    }
                    .padding(.horizontal)

                    // Category Selection
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Categories")
                            .font(.headline)
                            .foregroundColor(SpacedriveColors.Text.primary)
                            .padding(.horizontal)

                        ForEach(IconCategory.allCases, id: \.self) { category in
                            Button(action: {
                                selectedCategory = category
                            }) {
                                HStack {
                                    Text(category.rawValue)
                                    Spacer()
                                    if selectedCategory == category {
                                        Image(systemName: "checkmark")
                                            .foregroundColor(.accentColor)
                                    }
                                }
                                .padding(.horizontal)
                                .padding(.vertical, 4)
                            }
                            .buttonStyle(PlainButtonStyle())
                            .background(selectedCategory == category ? Color.accentColor.opacity(0.1) : Color.clear)
                            .cornerRadius(6)
                            .padding(.horizontal)
                        }
                    }

                    Divider()

                    // Size Selection
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Icon Size")
                            .font(.headline)
                            .foregroundColor(SpacedriveColors.Text.primary)
                            .padding(.horizontal)

                        ForEach(IconSize.allCases, id: \.self) { size in
                            Button(action: {
                                selectedSize = size
                            }) {
                                HStack {
                                    Text(size.rawValue)
                                    Spacer()
                                    if selectedSize == size {
                                        Image(systemName: "checkmark")
                                            .foregroundColor(.accentColor)
                                    }
                                }
                                .padding(.horizontal)
                                .padding(.vertical, 4)
                            }
                            .buttonStyle(PlainButtonStyle())
                            .background(selectedSize == size ? Color.accentColor.opacity(0.1) : Color.clear)
                            .cornerRadius(6)
                            .padding(.horizontal)
                        }
                    }

                    Divider()

                    // Variant Filters
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Show Variants")
                            .font(.headline)
                            .foregroundColor(SpacedriveColors.Text.primary)
                            .padding(.horizontal)

                        Toggle("Light Theme", isOn: $showLightVariants)
                            .padding(.horizontal)

                        Toggle("20px Variants", isOn: $show20pxVariants)
                            .padding(.horizontal)
                    }

                    Spacer()

                    // Stats
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Showing \(filteredIcons.count) icons")
                            .font(.caption)
                            .foregroundColor(SpacedriveColors.Text.secondary)
                            .padding(.horizontal)
                    }
                }
                .frame(width: 280)
                .background(SpacedriveColors.Background.secondary)

                // Main Content
                ScrollView {
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 16), count: 5), spacing: 20) {
                        ForEach(filteredIcons, id: \.self) { icon in
                            IconShowcaseCard(
                                icon: icon,
                                size: selectedSize.size,
                                showLightVariants: showLightVariants,
                                show20pxVariants: show20pxVariants
                            )
                        }
                    }
                    .padding(24)
                }
                .background(SpacedriveColors.Background.primary)
            }
            .navigationTitle("Spacedrive Icons")
            .frame(minWidth: 800, minHeight: 600)
        }
    }
}

/// Individual icon card in the showcase
struct IconShowcaseCard: View {
    let icon: SpacedriveIcon
    let size: CGFloat
    let showLightVariants: Bool
    let show20pxVariants: Bool
    @State private var isHovered = false

    var body: some View {
        VStack(spacing: 8) {
            // Icon display
            SpacedriveIconView(icon, size: size)
                .frame(width: size + 8, height: size + 8)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(isHovered ? Color.accentColor.opacity(0.1) : Color.clear)
                )

            // Variant indicators
            HStack(spacing: 4) {
                if icon.isLightVariant {
                    Text("L")
                        .font(.caption2)
                        .foregroundColor(.white)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 2)
                        .background(Color.blue)
                        .cornerRadius(4)
                }

                if icon.is20pxVariant {
                    Text("20")
                        .font(.caption2)
                        .foregroundColor(.white)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 2)
                        .background(Color.green)
                        .cornerRadius(4)
                }
            }

            // Filename
            Text(icon.filename)
                .font(.caption2)
                .foregroundColor(SpacedriveColors.Text.secondary)
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(SpacedriveColors.Background.tertiary)
                .shadow(color: Color.black.opacity(0.1), radius: 2, x: 0, y: 1)
        )
        .scaleEffect(isHovered ? 1.02 : 1.0)
        .animation(.easeInOut(duration: 0.2), value: isHovered)
        .onHover { hovering in
            isHovered = hovering
        }
        .help("\(icon.displayName)\n\(icon.filename)")
    }
}

/// Preview for the icon showcase
struct IconShowcaseView_Previews: PreviewProvider {
    static var previews: some View {
        IconShowcaseView()
            .environmentObject(SharedAppState.shared)
            .frame(width: 1000, height: 800)
    }
}
