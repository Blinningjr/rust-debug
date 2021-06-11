pub mod debug_request;
pub mod debug_response;
pub mod debug_event;
pub mod commands;


pub enum Command {
    Request(debug_request::DebugRequest),
    Response(debug_response::DebugResponse),
    Event(debug_event::DebugEvent),
}

