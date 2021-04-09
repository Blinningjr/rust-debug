use anyhow::{
    Result,
};

use crate::server::{
    Session,
    send_data,
    get_arguments,
};

use std::io::{
    Read,
    Write,
};

use serde_json::{
    json,
    to_vec,
};

use serde::{
    de::DeserializeOwned,
    Deserialize,
};

use debugserver_types::{
    Response,
    Request,
    ThreadsResponseBody,
    Thread,
    StoppedEventBody,
    StoppedEvent,
    StackTraceResponseBody,
    ContinueResponseBody,
    DisconnectArguments,
    SetBreakpointsArguments,
    SetBreakpointsResponseBody,
    SourceBreakpoint,
    Breakpoint,
};

use log::{
    debug,
    error,
    info,
    trace,
    warn,
};

use std::time::Duration;

use super::{
    read_dwarf,
    attach_probe,
};

use std::path::PathBuf;

impl<R: Read, W: Write> Session<R, W> {
    pub fn launch_command_request(&mut self, req: &Request) -> Result<bool> 
    {
        // TODO start the debugee
        unimplemented!();

        let resp = Response {
            body:           None,
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };
        
        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;
    
        Ok(false)
    }


    pub fn attach_command_request(&mut self, req: &Request) -> Result<bool> 
    {
        let args: AttachRequestArguments = get_arguments(&req)?;
        debug!("attach args: {:#?}", args);

        info!("> program: {:?}", args.program);
        self.file_path = Some(PathBuf::from(args.program));

        self.dwarf = Some(read_dwarf(self.file_path.as_ref().unwrap())?);
        debug!("> Read dwarf file");

        match attach_probe() {
            Ok(s) => {
                info!("> probe attached");
                self.sess = Some(s);

                let resp = Response {
                    body:           None,
                    command:        req.command.clone(),
                    message:        None,
                    request_seq:    req.seq,
                    seq:            req.seq,
                    success:        true,
                    type_:          "response".to_string(),
                };
                
                self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;

                // TODO: Add breakpoints

                return Ok(false);
            },
            Err(e) => {
                warn!("> probe failed to attach");
                return Err(e);
            },
        };
    }


    pub fn threads_command_request(&mut self, req: &Request) -> Result<bool> 
    {
        let body = ThreadsResponseBody {
            threads: vec!(Thread {
                id:     0,
                name:   "Main Thread".to_string(),
            }),
        };

        let resp = Response {
            body:           Some(json!(body)),
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };
        
        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?; 

        Ok(false)
    }


    pub fn configuration_done_command_request(&mut self, req: &Request) -> Result<bool>
    {
        let resp = Response {
            body:           None,
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };
        
        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;
    
        Ok(false)
    }


    pub fn pause_command_request(&mut self, req: &Request) -> Result<bool>
    {
        let resp = Response {
            body:           None,
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };
        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;

        match self.halt_core() {
            Ok(_) => {
                let body = StoppedEventBody {
                    reason: "pause".to_owned(),
                    description: Some("Target paused due to pause request.".to_owned()),
                    thread_id: Some(0),
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                };

                self.seq = send_data(&mut self.writer,
                                     &to_vec(&StoppedEvent {
                                        body:   body,
                                        event:  "stopped".to_owned(),
                                        seq:    self.seq,
                                        type_:  "event".to_owned(),
                                    })?,
                                    self.seq)?;
            },
            Err(err) => {
                warn!("Faild to halt target");
                trace!("Faild to halt target because: {}", err);
            }
        }
 
        Ok(false)
    }


    pub fn disconnect_command_request(&mut self, req: &Request) -> Result<bool>
    {
        let args: DisconnectArguments = get_arguments(&req)?;
        debug!("args: {:?}", args);
        // TODO: Stop the debuggee, if conditions are meet

        let resp = Response {
            body:           None,
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };
        
        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;
    
        Ok(true)
    }


    pub fn stack_trace_command_request(&mut self, req: &Request) -> Result<bool>
    {
        // TODO: Follow DAP spec
        
        let body = StackTraceResponseBody {
            stack_frames: vec!(),
            total_frames: None,
        };

        let resp = Response {
            body:           Some(json!(body)),
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };
        
        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;
    
        Ok(false)
    }


    pub fn continue_command_request(&mut self, req: &Request) -> Result<bool>
    {
        let _res = self.run_core();
        
        let body = ContinueResponseBody {
            all_threads_continued: Some(true),
        };
        
        let resp = Response {
            body:           Some(json!(body)),
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };
        
        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;
    
        Ok(false)
    }


    pub fn next_command_request(&mut self, req: &Request) -> Result<bool>
    {
        let resp = Response {
            body:           None,
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };
        
        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;

        match self.step_core() {
            Ok(_)      => {
                // TODO: send Stopped event.
                let body = StoppedEventBody {
                    reason: "step".to_owned(),
                    description: Some("Target paused due to step request.".to_owned()),
                    thread_id: Some(0),
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                };

                self.seq = send_data(&mut self.writer,
                                     &to_vec(&StoppedEvent {
                                        body:   body,
                                        event:  "stopped".to_owned(),
                                        seq:    self.seq,
                                        type_:  "event".to_owned(),
                                    })?,
                                    self.seq)?;
            },
            Err(err)    => {
                warn!("Faild to step");
                trace!("Faild to step because: {}", err);
            },
        };
    
        Ok(false)
    }


    pub fn set_breakpoints_command_request(&mut self, req: &Request) -> Result<bool>
    {
        let args: SetBreakpointsArguments = get_arguments(req)?;

        let breakpoints = match args.breakpoints {
            Some(bkpts) => bkpts,
            None        => vec!(),
        };

        debug!("source: {:#?}", args.source);
        debug!("sourceModified: {:#?}", args.source_modified);
        debug!("BreakPoints: {:#?}", breakpoints);

        let body = SetBreakpointsResponseBody {
            breakpoints: vec!(),
        };

        let resp = Response {
            body:           Some(json!(body)),
            command:        req.command.clone(),
            message:        None,
            request_seq:    req.seq,
            seq:            req.seq,
            success:        true,
            type_:          "response".to_string(),
        };

        self.seq = send_data(&mut self.writer, &to_vec(&resp)?, self.seq)?;

        Ok(false)
    }
}


#[derive(Deserialize, Debug, Default)]
struct AttachRequestArguments {
    program: String,
    chip: String,
    cwd: Option<String>,
    reset: Option<bool>,
    halt_after_reset: Option<bool>,
}

