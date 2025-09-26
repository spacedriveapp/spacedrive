import SwiftUI
import Combine

/// Development Window Configuration Helper
/// This file provides easy-to-use functions for switching between different window configurations during development.
///
/// Usage:
/// - Change the `currentDevConfiguration` variable below to switch between different setups
/// - Rebuild and run to see the new configuration
/// - Use the DevWindowToggleView for runtime switching

// MARK: - Quick Toggle Configuration
// 🔧 CHANGE THIS VALUE TO SWITCH WINDOW CONFIGURATIONS 🔧
// Available options: .default, .browserOnly, .companionOnly, .allWindows, .compact, .development
let currentDevConfiguration: DevWindowConfiguration = .development

// MARK: - Development Window Manager

@MainActor
class DevWindowManager: ObservableObject {
    static let shared = DevWindowManager()

    @Published var currentConfiguration: DevWindowConfiguration = currentDevConfiguration

    private init() {
        // Set the initial configuration in SharedAppState
        SharedAppState.shared.setDevWindowConfiguration(currentDevConfiguration)
    }

    func switchTo(_ config: DevWindowConfiguration) {
        currentConfiguration = config
        SharedAppState.shared.setDevWindowConfiguration(config)

        // Close all existing windows
        SharedAppState.shared.dispatch(.closeAllWindows)

        // Open new windows after a brief delay
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
            SharedAppState.shared.dispatch(.openDevWindows)
        }

        print("🔧 Switched to development configuration: \(config.displayName)")
    }

    func getConfigurationDescription() -> String {
        return currentConfiguration.description
    }
}

// MARK: - Development Window Toggle View

struct DevWindowToggleView: View {
    @StateObject private var devManager = DevWindowManager.shared
    @State private var showingConfiguration = false

    var body: some View {
        VStack(spacing: 12) {
            // Current Configuration Display
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Dev Config")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text(devManager.currentConfiguration.displayName)
                        .font(.headline)
                        .foregroundColor(.primary)
                }

                Spacer()

                Button("Switch") {
                    showingConfiguration.toggle()
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }

            // Configuration Description
            Text(devManager.getConfigurationDescription())
                .font(.caption)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.leading)
        }
        .padding()
        .background(Color.gray.opacity(0.1))
        .cornerRadius(8)
        .sheet(isPresented: $showingConfiguration) {
            DevConfigurationSelector()
        }
    }
}

struct DevConfigurationSelector: View {
    @Environment(\.dismiss) private var dismiss
    @StateObject private var devManager = DevWindowManager.shared

    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                Text("Development Window Configuration")
                    .font(.title2)
                    .fontWeight(.semibold)
                    .padding(.top)

                Text("Choose a window configuration for development. This will close all current windows and open the selected configuration.")
                    .font(.body)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)

                LazyVGrid(columns: [
                    GridItem(.flexible()),
                    GridItem(.flexible())
                ], spacing: 16) {
                    ForEach(DevWindowConfiguration.allCases, id: \.self) { config in
                        DevConfigCard(
                            configuration: config,
                            isSelected: devManager.currentConfiguration == config
                        ) {
                            devManager.switchTo(config)
                            dismiss()
                        }
                    }
                }
                .padding(.horizontal)

                Spacer()
            }
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
        .frame(width: 600, height: 500)
    }
}

struct DevConfigCard: View {
    let configuration: DevWindowConfiguration
    let isSelected: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            VStack(alignment: .leading, spacing: 8) {
                Text(configuration.displayName)
                    .font(.headline)
                    .foregroundColor(isSelected ? .white : .primary)
                    .multilineTextAlignment(.leading)

                Text(configuration.description)
                    .font(.caption)
                    .foregroundColor(isSelected ? .white.opacity(0.8) : .secondary)
                    .multilineTextAlignment(.leading)

                Spacer()
            }
            .padding()
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: 120)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(isSelected ? Color.blue : Color.gray.opacity(0.1))
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(isSelected ? Color.blue : Color.clear, lineWidth: 2)
                    )
            )
        }
        .buttonStyle(PlainButtonStyle())
    }
}

// MARK: - Quick Access Functions

/// Quick function to switch to browser-only mode
@MainActor
func switchToBrowserOnly() {
    DevWindowManager.shared.switchTo(.browserOnly)
}

/// Quick function to switch to development mode (browser + inspector)
@MainActor
func switchToDevelopmentMode() {
    DevWindowManager.shared.switchTo(.development)
}

/// Quick function to switch to companion-only mode
@MainActor
func switchToCompanionOnly() {
    DevWindowManager.shared.switchTo(.companionOnly)
}

/// Quick function to open all windows
@MainActor
func openAllWindows() {
    DevWindowManager.shared.switchTo(.allWindows)
}

// MARK: - Window Management Extensions
// (Extensions removed to avoid redeclaration conflicts)
