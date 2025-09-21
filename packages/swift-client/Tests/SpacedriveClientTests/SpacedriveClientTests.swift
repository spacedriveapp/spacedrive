import XCTest
@testable import SpacedriveClient

final class SpacedriveClientTests: XCTestCase {

    func testClientInitialization() {
        let client = SpacedriveClient(socketPath: "/tmp/test.sock")
        XCTAssertNotNil(client)
    }

    func testErrorTypes() {
        let error = SpacedriveError.connectionFailed("Test error")
        XCTAssertNotNil(error.errorDescription)
        XCTAssertTrue(error.errorDescription!.contains("Test error"))
    }

    // More tests will be added once the daemon connection is implemented
}
