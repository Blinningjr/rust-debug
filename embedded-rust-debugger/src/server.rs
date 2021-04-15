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
    SourceBreakpoint,
    Breakpoint,
};

use std::path::{
    PathBuf,
    Path,
};

use std::io;
use std::io::{BufRead, BufReader};
use std::io::{Read, Write};

use std::str::FromStr;
use std::string::ParseError;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

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

use gimli::{
    Dwarf,
    EndianRcSlice,
    LittleEndian,
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

#[derive(Debug)]
pub struct BreakpointInfo {
    id: u32,
    verified: bool,
    info: SourceBreakpoint,
    address: Option<u64>,
    location: Option<u32>,
}


#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEventBody {
    pub all_threads_stopped: Option<bool>,
    pub description: Option<String>,
    pub preserve_focus_hint: Option<bool>,
    pub reason: String,
    pub text: Option<String>,
    pub thread_id: Option<i64>,
    pub hit_breakpoint_ids: Option<Vec<u32>>,
}


//#[derive(Debug)]
pub struct Session<R: Read, W: Write> {
    pub reader: BufReader<R>,
    pub writer: W,
    pub seq:    i64,
    pub sess:   Option<probe_rs::Session>,
    pub file_path:  Option<PathBuf>,
    pub dwarf:  Option<Dwarf<EndianRcSlice<LittleEndian>>>,
    pub breakpoints: Vec<BreakpointInfo>,
    pub bkpt_id: u32,
    pub status: bool,
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
            breakpoints: vec!(),
            bkpt_id: 0,
            status: false,
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
            if self.status {
                self.check_bkpt();
            }
        }
    }

    fn check_bkpt(&mut self) -> Result<()>
    {
        if let Some(s) = &mut self.sess {
            let mut core = s.core(0)?;

            if commands::hit_breakpoint(&mut core)? {
                self.status = false;

                let pc = core.read_core_reg(core.registers().program_counter())?;

                let mut hit_breakpoint_ids = vec!();
                for bkpt in &self.breakpoints {
                    if let Some(loc) = bkpt.location {
                        if loc == pc {
                            hit_breakpoint_ids.push(bkpt.id);
                        }
                    }
                }

                let body = StoppedEventBody { 
                    reason: "breakpoint".to_owned(),
                    description: Some("Target stopped due to breakpoint.".to_owned()),
                    thread_id: Some(0),
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                    hit_breakpoint_ids: Some(hit_breakpoint_ids),
                };

                self.seq = send_data(&mut self.writer,
                                     &to_vec(&Event {
                                        body:   Some(json!(body)),
                                        event:  "stopped".to_owned(),
                                        seq:    self.seq,
                                        type_:  "event".to_owned(),
                                    })?,
                                    self.seq)?;
            }
        }

        Ok(())
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
            self.status = false;

            return Ok(());
        } else {
            return Err(anyhow!("Not attached to target"));
        } 
    }
    

    pub fn run_core(&mut self) -> Result<()> {
        if let Some(s) = &mut self.sess {
            let mut core = s.core(0)?;
            
            let bkpts = self.breakpoints.iter().filter(|bkpt| bkpt.verified).map(|bkpt| bkpt.location.unwrap()).collect();
            let _res = commands::run_command(&mut core, &bkpts)?;
            self.status = true;

            return Ok(());
        } else {
            return Err(anyhow!("Not attached to target"));
        } 
    }


    pub fn step_core(&mut self) -> Result<()> {
        if let Some(s) = &mut self.sess {
            let mut core = s.core(0)?;
           
            let bkpts = self.breakpoints.iter().filter(|bkpt| bkpt.verified).map(|bkpt| bkpt.location.unwrap()).collect();
            let _res = commands::step_command(&mut core, &bkpts, false)?;

            return Ok(());
        } else {
            return Err(anyhow!("Not attached to target"));
        } 
    }

    
    pub fn set_breakpoint(&mut self, bkpt: &SourceBreakpoint, source_location: Option<u64>, location: u32) -> Result<bool>
    {
        let verified = if let Some(mut core) =
            self.sess.as_mut().and_then(|s| s.core(0).ok())
        {
            core.set_hw_breakpoint(location)?;
            debug!("Breakpoint set at: {:?}", location);
            true
        } else {
            false
        };

        let id = self.bkpt_id;
        self.bkpt_id += 1;

        self.breakpoints.push(BreakpointInfo {
            id,
            verified,
            info:       bkpt.to_owned(),
            address:    source_location,
            location:   Some(location),
        });

        Ok(verified)
    }

    pub fn clear_all_breakpoints(&mut self) -> Result<()>
    {
        if let Some(session) = &mut self.sess {
            let mut core = session.core(0).ok().unwrap();

            for bkpt in &self.breakpoints {
                if bkpt.verified {
                    core.clear_hw_breakpoint(bkpt.location.unwrap())?;
                }
            }

            self.breakpoints = vec!();
        } else {
            return Err(anyhow!("Probe not attached"));
        }
        
        Ok(())
    }


    pub fn update_breakpoints(&mut self,
                              breakpoints: Vec<SourceBreakpoint>,
                              raw_path: Option<String>
                              ) -> Result<Vec<Breakpoint>>
    {
        let mut new_breakpoints = Vec::new();
        let source_path = raw_path.as_ref().map(Path::new);

        if let Some(path) = &self.file_path {
            let debug_info = probe_rs::debug::DebugInfo::from_file(path)?;

            self.clear_all_breakpoints()?;
            
            for bkpt in breakpoints {
                debug!(
                    "Trying to set breakpoint {:?}, source_file {:?}",
                    bkpt, source_path
                );

                let source_location: Option<u64> = debug_info.get_breakpoint_location(
                            dbg!(source_path.unwrap()),
                            dbg!(bkpt.line as u64),
                            bkpt.column.map(|c| c as u64),
                        )
                        .unwrap_or(None);

                if let Some(location) = source_location {
                    debug!("Found source location: {:#08x}!", location);

                    let verified = self.set_breakpoint(&bkpt, source_location, location as u32)?;

                    new_breakpoints.push(Breakpoint {
                        column: bkpt.column,
                        end_column: None,
                        end_line: None,
                        id: None,
                        line: Some(bkpt.line),
                        message: None,
                        source: None,
                        verified,
                    });
                } else {
                       warn!("Could not find brekpoint location {:?}", bkpt);

                       new_breakpoints.push(Breakpoint {
                           column: bkpt.column,
                           end_column: None,
                           end_line: None,
                           id: None,
                           line: Some(bkpt.line),
                           message: None,
                           source: None,
                           verified: false,
                       });
                }
            } 
        }
        
        return Ok(new_breakpoints);
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

