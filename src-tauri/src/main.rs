fn main() {
    // Process startup, before Tauri starts application threads or loads ONNX.
    // The API-level opt-out alone does not prevent the POSIX uploader starting.
    std::env::set_var("ORT_DISABLE_TELEMETRY", "1");
    knowledge_garden::desktop::run();
}
