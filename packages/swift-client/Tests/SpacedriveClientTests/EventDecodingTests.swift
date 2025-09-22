import XCTest
@testable import SpacedriveClient

final class EventDecodingTests: XCTestCase {

    func testJobStartedEventDecoding() throws {
        // This is the exact JSON that the daemon sends (inner event)
        let jsonString = """
        {
          "JobStarted": {
            "job_id": "8525ff04-3025-409a-a98f-e94737bd94d4",
            "job_type": "Indexing"
          }
        }
        """

        let jsonData = jsonString.data(using: .utf8)!

        do {
            let event = try JSONDecoder().decode(Event.self, from: jsonData)
            print("✅ Successfully decoded event: \(event)")

            // Check if it's the right type
            if case .jobStarted(let data) = event {
                XCTAssertEqual(data.jobId, "8525ff04-3025-409a-a98f-e94737bd94d4")
                XCTAssertEqual(data.jobType, "Indexing")
                print("✅ Event data is correct: jobId=\(data.jobId), jobType=\(data.jobType)")
            } else {
                XCTFail("Event was not JobStarted type")
            }
        } catch {
            print("❌ Decoding failed: \(error)")
            XCTFail("Failed to decode JobStarted event: \(error)")
        }
    }

    func testDaemonResponseDecoding() throws {
        // This is the exact JSON that the daemon sends (wrapped format)
        let jsonString = """
        {"Event":{"JobStarted":{"job_id":"8525ff04-3025-409a-a98f-e94737bd94d4","job_type":"Indexing"}}}
        """

        let jsonData = jsonString.data(using: .utf8)!

        do {
            let response = try JSONDecoder().decode(DaemonResponse.self, from: jsonData)
            print("✅ Successfully decoded daemon response: \(response)")

            // Check if it's the right type
            if case .event(let event) = response {
                print("✅ Extracted event from response: \(event)")

                if case .jobStarted(let data) = event {
                    XCTAssertEqual(data.jobId, "8525ff04-3025-409a-a98f-e94737bd94d4")
                    XCTAssertEqual(data.jobType, "Indexing")
                    print("✅ Event data is correct: jobId=\(data.jobId), jobType=\(data.jobType)")
                } else {
                    XCTFail("Event was not JobStarted type")
                }
            } else {
                XCTFail("Response was not Event type")
            }
        } catch {
            print("❌ Decoding failed: \(error)")
            XCTFail("Failed to decode daemon response: \(error)")
        }
    }
}
