use std::io::prelude::*;
use std::net::{
    TcpListener,
    TcpStream,
    SocketAddr,
};


use anyhow::{
    anyhow,
    Result,
};

use log::{
    debug,
    error,
    info,
    trace,
};

use simplelog::*;

use debugserver_types::{
    ProtocolMessage,
    Response,
    Request,
    InitializeRequestArguments,
    Capabilities,
    InitializedEvent,
};

use std::io;
use std::io::{BufRead, BufReader};
use std::io::{Read, Write};

use std::str::FromStr;
use std::string::ParseError;

use serde::{de::DeserializeOwned, Deserialize};

use serde_json::{
    from_slice,
    from_value,
    json,
    to_vec,
};


pub fn start_server(port: u16) -> Result<(), anyhow::Error>
{
    info!("Starting debug-adapter server on port: {}", port);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr)?;

    let (socket, addr) = listener.accept()?;
    info!("Accepted connection from {}", addr);

    let reader = BufReader::new(socket.try_clone()?);
    let writer = socket;

    start_session(reader, writer)
}


fn start_session<R: Read, W: Write>(mut reader: BufReader<R>, mut writer: W) -> Result<()>
{
    let req = verify_init_msg(read_dap_msg(&mut reader)?)?;

    let capabilities = Capabilities {..Default::default()};
    let resp = Response {
        body:           Some(json!(capabilities)),
        command:        req.command.clone(),
        message:        None,
        request_seq:    req.seq,
        seq:            req.seq,
        success:        true,
        type_:          "response".to_string(),
    };
    
    let mut seq = send_data(&mut writer, &to_vec(&resp)?, 0)?;
    seq = send_data(&mut writer,
                    &to_vec(&InitializedEvent {
                        seq,
                        body: None,
                        type_: "event".to_owned(),
                        event: "initialized".to_owned(),
                    })?,
                    seq)?;


    loop {
        let msg = read_dap_msg(&mut reader)?;
        trace!("< {:?}", msg);
    }


    //Ok(())
}


fn verify_init_msg(msg: DebugAdapterMessage) -> Result<Request>
{
    match msg {
        DebugAdapterMessage::Request(req)   => {
            if req.command != "initialize" {
                return Err(anyhow!("Error: Expected command initialize got {}", req.command));
            }
            
            let arguments: InitializeRequestArguments = get_arguments(&req)?;
            debug!("Initialization request from client '{}'",
                   arguments.client_name.unwrap_or("<unknown>".to_owned()));
            Ok(req)
        },

        _                                   =>
            Err(anyhow!("Error: initial message should be of type request")),
    }
}



fn read_dap_msg<R: Read>(reader: &mut BufReader<R>) -> Result<DebugAdapterMessage, anyhow::Error>
{
    let mut header = String::new();

    reader.read_line(&mut header)?;
    trace!("< {}", header.trim_end());

    // we should read an empty line here
    let mut buff = String::new();
    reader.read_line(&mut buff)?;

    let len = get_content_len(&header)
        .ok_or_else(|| anyhow!("Failed to read content length from header '{}'", header))?;

    let mut content = vec![0u8; len];
    let bytes_read = reader.read(&mut content)?;

    // Extract protocol message
    let protocol_msg: ProtocolMessage = from_slice(&content)?;
    trace!("{:#?}", protocol_msg);

    let msg = match protocol_msg.type_.as_ref() {
        "request" => DebugAdapterMessage::Request(from_slice(&content,)?),
        "response" => DebugAdapterMessage::Response(from_slice(&content,)?),
        "event" => DebugAdapterMessage::Event(from_slice(&content,)?),
        other => return Err(anyhow!("Unknown message type: {}", other)),
    };
    debug!("Got msg: {:?}", msg);
    Ok(msg)
}


fn get_content_len(header: &str) -> Option<usize> {
    let mut parts = header.trim_end().split_ascii_whitespace();

    // discard first part
    parts.next()?;
    parts.next()?.parse::<usize>().ok()
}


#[derive(Debug)]
pub enum DebugAdapterMessage {
    Request(Request),
    Response(Response),
    Event(debugserver_types::Event),
}


pub fn get_arguments<T: DeserializeOwned>(req: &Request) -> Result<T> {
    let value = req.arguments.as_ref().unwrap();
    from_value(value.to_owned()).map_err(|e| e.into())
}


fn send_data<W: Write>(writer: &mut W, raw_data: &[u8], seq: i64) -> Result<i64> {
    let resp_body = raw_data;

    let resp_header = format!("Content-Length: {}\r\n\r\n", resp_body.len());

    trace!("> {}", resp_header.trim_end());
    trace!("> {}", std::str::from_utf8(resp_body)?);

    writer.write(resp_header.as_bytes())?;
    writer.write(resp_body)?;

    writer.flush()?;

    Ok(seq + 1)
}

