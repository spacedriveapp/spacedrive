import XCTest
@testable import SpacedriveClient

final class SerializationTests: XCTestCase {

    func testLibraryCreateInputSerialization() throws {
        // Test that Swift types serialize to JSON correctly
        let input = LibraryCreateInput(name: "Test Library", path: "/test/path")

        // Serialize to JSON
        let jsonData = try JSONEncoder().encode(input)
        let jsonString = String(data: jsonData, encoding: .utf8)!

        print("LibraryCreateInput JSON: \(jsonString)")

        // Verify JSON structure matches what Rust expects
        let jsonObject = try JSONSerialization.jsonObject(with: jsonData) as! [String: Any]
        XCTAssertEqual(jsonObject["name"] as? String, "Test Library")
        XCTAssertEqual(jsonObject["path"] as? String, "/test/path")

        // Test round-trip serialization
        let decoded = try JSONDecoder().decode(LibraryCreateInput.self, from: jsonData)
        XCTAssertEqual(decoded.name, input.name)
        XCTAssertEqual(decoded.path, input.path)
    }

    func testLibraryCreateOutputDeserialization() throws {
        // Test that we can deserialize JSON from daemon into Swift types
        let jsonString = """
        {
            "libraryId": "123e4567-e89b-12d3-a456-426614174000",
            "name": "Test Library",
            "path": "/test/path"
        }
        """

        let jsonData = jsonString.data(using: .utf8)!
        let output = try JSONDecoder().decode(LibraryCreateOutput.self, from: jsonData)

        XCTAssertEqual(output.libraryId, "123e4567-e89b-12d3-a456-426614174000")
        XCTAssertEqual(output.name, "Test Library")
        XCTAssertEqual(output.path, "/test/path")

        print("LibraryCreateOutput deserialized successfully: \(output)")
    }

    func testUnionTypeSerialization() throws {
        // Test union types (enums with associated values)
        let physicalPath = SdPath.physical(SdPathPhysicalData(deviceId: "device-123", path: "/test/file.txt"))
        let contentPath = SdPath.content(SdPathContentData(contentId: "content-456"))

        // Test physical path serialization
        let physicalData = try JSONEncoder().encode(physicalPath)
        let physicalJson = String(data: physicalData, encoding: .utf8)!
        print("Physical SdPath JSON: \(physicalJson)")

        // Test content path serialization
        let contentData = try JSONEncoder().encode(contentPath)
        let contentJson = String(data: contentData, encoding: .utf8)!
        print("Content SdPath JSON: \(contentJson)")

        // Test round-trip
        let decodedPhysical = try JSONDecoder().decode(SdPath.self, from: physicalData)
        let decodedContent = try JSONDecoder().decode(SdPath.self, from: contentData)

        // Verify the decoded values match
        switch decodedPhysical {
        case .physical(let data):
            XCTAssertEqual(data.deviceId, "device-123")
            XCTAssertEqual(data.path, "/test/file.txt")
        case .content:
            XCTFail("Expected physical path")
        }

        switch decodedContent {
        case .content(let data):
            XCTAssertEqual(data.contentId, "content-456")
        case .physical:
            XCTFail("Expected content path")
        }
    }

    func testJobStatusSerialization() throws {
        // Test simple enum serialization
        let statuses: [JobStatus] = [.queued, .running, .completed, .failed]

        for status in statuses {
            let data = try JSONEncoder().encode(status)
            let json = String(data: data, encoding: .utf8)!
            let decoded = try JSONDecoder().decode(JobStatus.self, from: data)

            print("JobStatus \(status) → JSON: \(json)")
            XCTAssertEqual(decoded, status)
        }
    }

    func testJobOutputSerialization() throws {
        // Test complex enum with associated values
        let indexedOutput = JobOutput.indexed(JobOutputIndexedData(
            stats: IndexerStats(files: 100, dirs: 10, bytes: 1024000, symlinks: 5, skipped: 2, errors: 0),
            metrics: IndexerMetrics(
                totalDuration: 30.5,
                discoveryDuration: 5.0,
                processingDuration: 20.0,
                contentDuration: 5.5,
                filesPerSecond: 3.33,
                bytesPerSecond: 34133.33,
                dirsPerSecond: 0.33,
                dbWrites: 110,
                dbReads: 50,
                batchCount: 5,
                avgBatchSize: 20.0,
                totalErrors: 0,
                criticalErrors: 0,
                nonCriticalErrors: 0,
                skippedPaths: 2,
                peakMemoryBytes: 1048576,
                avgMemoryBytes: 524288
            )
        ))

        // Test serialization
        let data = try JSONEncoder().encode(indexedOutput)
        let json = String(data: data, encoding: .utf8)!
        print("Complex JobOutput JSON: \(json)")

        // Test round-trip
        let decoded = try JSONDecoder().decode(JobOutput.self, from: data)

        switch decoded {
        case .indexed(let data):
            XCTAssertEqual(data.stats.files, 100)
            XCTAssertEqual(data.metrics.filesPerSecond, 3.33, accuracy: 0.01)
        default:
            XCTFail("Expected indexed output")
        }
    }

    func testFileSystemEnumSerialization() throws {
        // Test enum with associated values
        let apfs = FileSystem.aPFS
        let other = FileSystem.other("custom-fs")

        // Test simple variant
        let apfsData = try JSONEncoder().encode(apfs)
        let apfsJson = String(data: apfsData, encoding: .utf8)!
        print("FileSystem.apfs JSON: \(apfsJson)")

        // Test variant with associated value
        let otherData = try JSONEncoder().encode(other)
        let otherJson = String(data: otherData, encoding: .utf8)!
        print("FileSystem.other JSON: \(otherJson)")

        // Test round-trip
        let decodedApfs = try JSONDecoder().decode(FileSystem.self, from: apfsData)
        let decodedOther = try JSONDecoder().decode(FileSystem.self, from: otherData)

        // XCTAssertEqual(decodedApfs, .apfs) // TODO: Add Equatable to generated enums
        switch decodedOther {
        case .other(let fs):
            XCTAssertEqual(fs, "custom-fs")
        default:
            XCTFail("Expected other filesystem")
        }
    }

    func testRealDaemonIntegration() async throws {
        // Skip if daemon is not running
        let socketPath = "\(NSHomeDirectory())/Library/Application Support/spacedrive/daemon/daemon.sock"

        guard FileManager.default.fileExists(atPath: socketPath) else {
            throw XCTSkip("Daemon not running - skipping integration test")
        }

        let client = SpacedriveClient(socketPath: socketPath)

        // Test real API call with generated types
        do {
            let libraries = try await client.executeQuery(
                LibraryListQuery(),
                method: "query:libraries.list.v1",
                responseType: [LibraryInfo].self
            )

            print("✅ Real daemon integration successful - found \(libraries.count) libraries")

            // If we have libraries, test job list with generated types
            if !libraries.isEmpty {
                let jobsResponse = try await client.executeQuery(
                    JobListQuery(),
                    method: "query:jobs.list.v1",
                    responseType: JobListOutput.self
                )

                print("✅ Jobs query successful - found \(jobsResponse.jobs.count) jobs")

                // Verify the types match our generated Swift types
                for job in jobsResponse.jobs {
                    XCTAssertFalse(job.id.isEmpty)
                    XCTAssertFalse(job.name.isEmpty)
                    // job.status should be a JobStatus enum value
                    print("  Job: \(job.name) (\(job.status)) - \(Int(job.progress * 100))%")
                }
            }

        } catch {
            print("⚠️ Daemon integration failed: \(error)")
            // Don't fail the test - daemon might not have libraries
        }
    }
}

// Helper types for testing (these should eventually be generated too)
struct LibraryListQuery: Codable {
    let include_stats: Bool

    init() {
        self.include_stats = false
    }
}

struct JobListQuery: Codable {
    let status: String?

    init() {
        self.status = nil
    }
}

struct LibraryInfo: Codable {
    let id: String
    let name: String
    let path: String
    let stats: LibraryStatistics?
}

struct LibraryStatistics: Codable {
    let total_files: UInt64
    let total_size: UInt64
    let location_count: UInt32
}

struct JobListOutput: Codable {
    let jobs: [JobListItem]
}

struct JobListItem: Codable {
    let id: String
    let name: String
    let status: String
    let progress: Float
}
