import Foundation

// MARK: - Simple JSON-RPC Protocol Types for iOS FFI Communication

/// JSON-RPC 2.0 Request structure for communication with embedded Rust core
internal struct JSONRPCRequest: Codable {
    let jsonrpc: String
    let method: String
    let params: JSONRPCParams
    let id: String

    init<T: Codable>(method: String, input: T, library_id: String?, id: String) throws {
        self.jsonrpc = "2.0"
        self.method = method
        self.params = try JSONRPCParams(input: input, library_id: library_id)
        self.id = id
    }
}

/// Parameters for JSON-RPC request
internal struct JSONRPCParams: Codable {
    let input: Data // Store as raw JSON data
    let library_id: String?

    enum CodingKeys: String, CodingKey {
        case input
        case library_id
    }

    init<T: Codable>(input: T, library_id: String?) throws {
        self.input = try JSONEncoder().encode(input)
        self.library_id = library_id
    }

    // Custom encoding to embed the input JSON directly
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)

        // Decode the stored JSON data and encode it as a raw JSON object
        let jsonObject = try JSONSerialization.jsonObject(with: input)
        try container.encode(AnyEncodableValue(jsonObject), forKey: .input)
        try container.encode(library_id, forKey: .library_id)
    }
}

/// JSON-RPC 2.0 Response structure
internal struct JSONRPCResponse<T: Codable>: Codable {
    let jsonrpc: String
    let id: String
    let result: T?
    let error: JSONRPCError?

    enum CodingKeys: String, CodingKey {
        case jsonrpc
        case id
        case result
        case error
    }
}

/// JSON-RPC Error structure
internal struct JSONRPCError: Codable {
    let code: Int
    let message: String
}

/// Helper for encoding arbitrary JSON values
private struct AnyEncodableValue: Encodable {
    let value: Any

    init(_ value: Any) {
        self.value = value
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()

        switch value {
        case let boolValue as Bool:
            try container.encode(boolValue)
        case let numberValue as NSNumber:
            // Handle NSNumber which can represent booleans as integers
            // Check if this NSNumber is actually a boolean
            if CFGetTypeID(numberValue) == CFBooleanGetTypeID() {
                try container.encode(numberValue.boolValue)
            } else if numberValue.doubleValue.truncatingRemainder(dividingBy: 1) == 0 {
                try container.encode(numberValue.intValue)
            } else {
                try container.encode(numberValue.doubleValue)
            }
        case let intValue as Int:
            try container.encode(intValue)
        case let doubleValue as Double:
            try container.encode(doubleValue)
        case let stringValue as String:
            try container.encode(stringValue)
        case let arrayValue as [Any]:
            try container.encode(arrayValue.map { AnyEncodableValue($0) })
        case let dictValue as [String: Any]:
            try container.encode(dictValue.mapValues { AnyEncodableValue($0) })
        case is NSNull:
            try container.encodeNil()
        default:
            throw EncodingError.invalidValue(
                value,
                EncodingError.Context(codingPath: encoder.codingPath, debugDescription: "Value is not encodable")
            )
        }
    }
}
