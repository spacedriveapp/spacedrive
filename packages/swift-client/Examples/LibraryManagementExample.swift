import Foundation
import SpacedriveClient

/// Example demonstrating the new library management features
@main
struct LibraryManagementExample {
    static func main() async {
        let client = SpacedriveClient(socketPath: "/tmp/spacedrive.sock")

        do {
            // Check initial state
            print("📚 Initial library status: \(client.getCurrentLibraryStatus())")
            print("📚 Has active library: \(client.hasActiveLibrary())")

            // Get list of available libraries
            let libraries = try await client.getLibraries()
            print("📚 Available libraries: \(libraries.map { "\($0.name) (\($0.id))" })")

            if let firstLibrary = libraries.first {
                // Switch to the first library by ID
                try await client.switchToLibrary(firstLibrary.id)
                print("📚 Switched to library: \(client.getCurrentLibraryStatus())")

                // Get jobs for the current library
                let jobs = try await client.getCurrentLibraryJobs()
                print("📚 Jobs in current library: \(jobs.jobs.count)")

                // Get current library info
                if let currentInfo = try await client.getCurrentLibraryInfo() {
                    print("📚 Current library details: \(currentInfo.name) at \(currentInfo.path)")
                }
            }

            // Create a new library and automatically switch to it
            let newLibrary = try await client.createAndSwitchToLibrary(
                name: "Example Library",
                path: "/tmp/example-library"
            )
            print("📚 Created and switched to new library: \(newLibrary.name) (\(newLibrary.libraryId))")

            // Switch to a library by name
            if libraries.count > 1 {
                try await client.switchToLibrary(named: libraries[1].name)
                print("📚 Switched to library by name: \(client.getCurrentLibraryStatus())")
            }

            // Clear the current library
            client.clearCurrentLibrary()
            print("📚 Cleared current library: \(client.getCurrentLibraryStatus())")

        } catch {
            print("❌ Error: \(error)")
        }
    }
}
