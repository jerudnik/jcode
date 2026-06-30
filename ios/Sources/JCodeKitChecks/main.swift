import Foundation
import JCodeKit

struct CheckFailure: Error, CustomStringConvertible {
    let message: String

    var description: String { message }
}

func expect(_ condition: Bool, _ message: String) throws {
    if !condition {
        throw CheckFailure(message: message)
    }
}

func encodedObject(_ request: Request) throws -> [String: Any] {
    let line = try request.encodedLine()
    let data = Data(line.utf8)
    guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        throw CheckFailure(message: "request did not encode to a JSON object: \(line)")
    }
    return object
}

actor FakeTransport: WebSocketTransport {
    enum Behavior {
        case succeed
        case failConnect
    }

    let behavior: Behavior
    private var sentLines: [String] = []
    private var incoming: [String] = []
    private var waiters: [CheckedContinuation<String?, Never>] = []
    private var closed = false

    init(behavior: Behavior = .succeed) {
        self.behavior = behavior
    }

    func connect(url: URL, authToken: String) async throws {
        if behavior == .failConnect {
            throw TransportError.notConnected
        }
    }

    func send(text: String) async throws {
        if closed { throw TransportError.notConnected }
        sentLines.append(text)
    }

    func receiveText() async throws -> String? {
        if closed { return nil }
        if !incoming.isEmpty {
            return incoming.removeFirst()
        }
        return await withCheckedContinuation { continuation in
            waiters.append(continuation)
        }
    }

    func close() async {
        closed = true
        for waiter in waiters {
            waiter.resume(returning: nil)
        }
        waiters.removeAll()
    }

    func push(_ line: String) {
        if let waiter = waiters.first {
            waiters.removeFirst()
            waiter.resume(returning: line)
        } else {
            incoming.append(line)
        }
    }

    func sent() -> [String] {
        sentLines
    }
}

func makeConnection(transport: FakeTransport) -> Connection {
    Connection(
        configuration: .init(
            gateway: Gateway(host: "test.local"),
            authToken: "tok",
            maxReconnectAttempts: 1,
            baseBackoffSeconds: 0.01
        ),
        makeTransport: { transport }
    )
}

func runReducer(_ lines: [String], from state: SessionState = SessionState()) throws -> SessionState {
    try lines.reduce(state) { state, line in
        try SessionReducer.reduce(state, .event(ServerEvent.decode(line: line)))
    }
}

func checkGatewayAndWire() throws {
    let gateway = Gateway(host: "devbox.tailnet.ts.net")
    try expect(gateway.healthURL.absoluteString == "http://devbox.tailnet.ts.net:7643/health", "health URL")
    try expect(gateway.pairURL.absoluteString == "http://devbox.tailnet.ts.net:7643/pair", "pair URL")
    try expect(gateway.webSocketURL.absoluteString == "ws://devbox.tailnet.ts.net:7643/ws", "ws URL")

    let payload = PairURI.parse("jcode://pair?host=mybox.ts.net&port=7643&code=123456")
    try expect(payload?.gateway.host == "mybox.ts.net", "pair URI host")
    try expect(payload?.gateway.port == 7643, "pair URI port")
    try expect(payload?.code == "123456", "pair URI code")

    let message = try encodedObject(.message(id: 7, content: "hello"))
    try expect(message["type"] as? String == "message", "message request type")
    try expect(message["id"] as? UInt64 == 7, "message request id")
    try expect(message["content"] as? String == "hello", "message request content")

    try expect(
        try ServerEvent.decode(line: #"{"type":"text_delta","text":"Hel"}"#) == .textDelta(text: "Hel"),
        "text_delta decode"
    )
    try expect(
        try ServerEvent.decode(line: #"{"type":"message_end"}"#) == .messageEnd,
        "message_end decode"
    )
}

func checkReducer() throws {
    var state = SessionReducer.reduce(SessionState(), intent: .userSentMessage("hi"))
    try expect(state.isProcessing, "user send marks processing")
    state = try runReducer([
        #"{"type":"text_delta","text":"hey"}"#,
        #"{"type":"done","id":1}"#,
    ], from: state)
    try expect(state.transcript.map(\.role) == [.user, .assistant], "user + assistant transcript")
    try expect(state.transcript[1].text == "hey", "assistant text")
    try expect(state.isProcessing == false, "done clears processing")

    let toolState = try runReducer([
        #"{"type":"tool_start","id":"t1","name":"bash"}"#,
        #"{"type":"tool_input","delta":"{\"command\":"}"#,
        #"{"type":"tool_input","delta":"\"ls\"}"}"#,
        #"{"type":"tool_exec","id":"t1","name":"bash"}"#,
        #"{"type":"tool_done","id":"t1","name":"bash","output":"file.txt"}"#,
    ])
    try expect(toolState.transcript.first?.toolCalls.first?.name == "bash", "tool name")
    try expect(toolState.transcript.first?.toolCalls.first?.output == "file.txt", "tool output")
}

func checkConnection() async throws {
    let transport = FakeTransport()
    let connection = makeConnection(transport: transport)
    let stream = await connection.start()
    var iterator = stream.makeAsyncIterator()

    try expect(await iterator.next() == .phase(.connecting), "connection starts connecting")
    try expect(await iterator.next() == .phase(.connected), "connection connects")

    var sent: [String] = []
    for _ in 0..<50 {
        sent = await transport.sent()
        if sent.count >= 2 { break }
        try await Task.sleep(nanoseconds: 10_000_000)
    }
    try expect(sent.count == 2, "connection sends subscribe + history")
    try expect(sent[0].contains("\"type\":\"subscribe\""), "subscribe sent")
    try expect(sent[1].contains("\"type\":\"get_history\""), "get_history sent")

    await transport.push(#"{"type":"text_delta","text":"hi"}"#)
    try expect(await iterator.next() == .event(.textDelta(text: "hi")), "connection decodes events")

    await connection.stop()
}

@main
struct JCodeKitChecks {
    static func main() async {
        do {
            try checkGatewayAndWire()
            try checkReducer()
            try await checkConnection()
            print("JCodeKitChecks: all checks passed")
        } catch {
            fputs("JCodeKitChecks failed: \(error)\n", stderr)
            Foundation.exit(1)
        }
    }
}
