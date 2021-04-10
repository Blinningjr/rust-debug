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
    warn,
};

use simplelog::*;

use debugserver_types::{
    ProtocolMessage,
    Response,
    Request,
    InitializeRequestArguments,
    Capabilities,
    InitializedEvent,
    Event,
};

use std::path::PathBuf;

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

use std::time::Duration;

use super::{
    commands,
};



pub fn start_server(port: u16) -> Result<(), anyhow::Error>
{
    info!("Starting debug-adapter server on port: {}", port);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr)?;

    loop {
        let (socket, addr) = listener.accept()?;
        info!("Accepted connection from {}", addr);

        let reader = BufReader::new(socket.try_clone()?);
        let writer = socket;

        Session::start_session(reader, writer);
    }

    Ok(())
}



//#[derive(Debug)]
pub struct Session<R: Read, W: Write> {
    pub reader: BufReader<R>,
    pub writer: W,
    pub seq:    i64,
    pub sess:   Option<probe_rs::Session>,
    pub file_path:  Option<PathBuf>,
    pub dwarf:  Option<gimli::Dwarf<Vec<u8>>>,
}

impl<R: Read, W: Write> Session<R, W> {
    fn start_session(mut reader: BufReader<R>, mut writer: W) -> Result<()>
    {
        let req = verify_init_msg(read_dap_msg(&mut reader)?)?;
    
        let capabilities = Capabilities {
            supports_configuration_done_request:    Some(true), // Supports config after init request
//            supports_data_breakpoints:              Some(true),
    //        supportsCancelRequest:                  Some(true),
            ..Default::default()
        };
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
                            seq:    seq,
                            body:   None,
                            type_:  "event".to_owned(),
                            event:  "initialized".to_owned(),
                        })?,
                        seq)?;


        let mut session = Session {
            reader: reader,
            writer: writer,
            seq:    seq,
            sess:   None,
            file_path: None,
            dwarf:  None,
        };
 
        session.run() 
    }

    fn run(&mut self) -> Result<()>
    {
        loop {
            let msg = read_dap_msg(&mut self.reader)?;
            trace!("< {:?}", msg);
            if self.handle_message(msg)? {
                return Ok(());
            }
        }
    }

    fn handle_message(&mut self, msg: DebugAdapterMessage) -> Result<bool>
    {
        match msg {
            DebugAdapterMessage::Request    (req)   => self.handle_request(req),
            DebugAdapterMessage::Response   (resp)  => unimplemented!(), 
            DebugAdapterMessage::Event      (event) => unimplemented!(),
        }
    }


    fn handle_request(&mut self, req: Request) -> Result<bool>
    {
        let res = match req.command.as_ref() {
            "launch"                    => self.launch_command_request(&req),
            "attach"                    => self.attach_command_request(&req),
            "setBreakpoints"            => self.set_breakpoints_command_request(&req),
            "threads"                   => self.threads_command_request(&req),
//            "setDataBreakpoints"        => Ok(()), // TODO
//            "setExceptionBreakpoints"   => Ok(()), // TODO
            "configurationDone"         => self.configuration_done_command_request(&req),
            "pause"                     => self.pause_command_request(&req),
            "stackTrace"                => self.stack_trace_command_request(&req),
            "disconnect"                => self.disconnect_command_request(&req),
            "continue"                  => self.continue_command_request(&req),
            "scopes"                    => unimplemented!(), // TODO
            "source"                    => unimplemented!(), // TODO
            "variables"                 => unimplemented!(), // TODO
            "next"                      => self.next_command_request(&req), // TODO
            "stepOut"                   => unimplemented!(), // TODO
            _ => panic!("command: {}", req.command),
        };

        match res {
            Ok(v)       => return Ok(v),
            Err(err)    => {
                warn!("{}", err.to_string());
                let resp = Response {
                    body:           None,
                    command:        req.command.clone(),
                    message:        Some(err.to_string()),
                    request_seq:    req.seq,
                    seq:            req.seq,
                    success:        false,
                    type_:          "response".to_string(),
                };
                
                self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?; 

                return Ok(false);
            },
        };
    }


    pub fn halt_core(&mut self) -> Result<()> {
        if let Some(s) = &mut self.sess {
            let mut core = s.core(0)?;
    
            let _res = commands::halt_command(&mut core, false)?;

            return Ok(());
        } else {
            return Err(anyhow!("Not attached to target"));
        } 
    }
    

    pub fn run_core(&mut self) -> Result<()> {
        if let Some(s) = &mut self.sess {
            let mut core = s.core(0)?;
            
            let _res = commands::run_command(&mut core)?;

            return Ok(());
        } else {
            return Err(anyhow!("Not attached to target"));
        } 
    }


    pub fn step_core(&mut self) -> Result<()> {
        if let Some(s) = &mut self.sess {
            let mut core = s.core(0)?;
            
            let _res = commands::step_command(&mut core, false)?;

            return Ok(());
        } else {
            return Err(anyhow!("Not attached to target"));
        } 
    }
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
    //trace!("< {}", header.trim_end());

    // we should read an empty line here
    let mut buff = String::new();
    reader.read_line(&mut buff)?;

    let len = get_content_len(&header)
        .ok_or_else(|| anyhow!("Failed to read content length from header '{}'", header))?;

    let mut content = vec![0u8; len];
    let bytes_read = reader.read(&mut content)?;

    // Extract protocol message
    let protocol_msg: ProtocolMessage = from_slice(&content)?;
    //trace!("{:#?}", protocol_msg);

    let msg = match protocol_msg.type_.as_ref() {
        "request" => DebugAdapterMessage::Request(from_slice(&content,)?),
        "response" => DebugAdapterMessage::Response(from_slice(&content,)?),
        "event" => DebugAdapterMessage::Event(from_slice(&content,)?),
        other => return Err(anyhow!("Unknown message type: {}", other)),
    };
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
    Event(Event),
}


pub fn get_arguments<T: DeserializeOwned>(req: &Request) -> Result<T> {
    let value = req.arguments.as_ref().unwrap();
    from_value(value.to_owned()).map_err(|e| e.into())
}


pub fn send_data<W: Write>(writer: &mut W, raw_data: &[u8], seq: i64) -> Result<i64> {
    let resp_body = raw_data;

    let resp_header = format!("Content-Length: {}\r\n\r\n", resp_body.len());

    //trace!("> {}", resp_header.trim_end());
    trace!("> {}", std::str::from_utf8(resp_body)?);

    writer.write(resp_header.as_bytes())?;
    writer.write(resp_body)?;

    writer.flush()?;

    Ok(seq + 1)
}
