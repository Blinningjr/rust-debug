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

use probe_rs;

use std::path::{
    PathBuf,
    Path,
};


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

        let dwarf_sections = read_dwarf(self.file_path.as_ref().unwrap())?;
        self.dwarf = Some(dwarf_sections);
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
        self.run_core()?;

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
        let mut stack_frames = vec!();
        if let (Some((dwarf, fs)), Some(sess)) = (&self.dwarf, &mut self.sess) {
            let mut core = sess.core(0)?;
            use crate::Debugger;
            let mut debugger = Debugger::new(dwarf, fs);
            let stacktrace = debugger.get_current_stacktrace(&mut core)?;

            for s in stacktrace {
                let source = debugserver_types::Source {
                    name: s.source.file,
                    path: s.source.directory,
                    source_reference: None,
                    presentation_hint: None,
                    origin: None,
                    sources: None,
                    adapter_data: None,
                    checksums: None,
                };
                stack_frames.push(debugserver_types::StackFrame {
                    id: s.call_frame.id as i64,
                    name: s.name,
                    source: Some(source),
                    line: s.source.line.unwrap() as i64,
                    column: match s.source.column {Some(v) => v as i64, None => 0,},
                    end_column: None,
                    end_line: None,
                    module_id: None,
                    presentation_hint: Some("normal".to_owned()),
                });
            }
        }
       
        let total_frames = stack_frames.len() as i64;
        let body = StackTraceResponseBody {
            stack_frames: stack_frames,
            total_frames: Some(total_frames),
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


    pub fn scopes_command_request(&mut self, req: &Request) -> Result<bool>
    {
        let args: debugserver_types::ScopesArguments = get_arguments(&req)?;
        debug!("args: {:?}", args);

        let mut scopes = vec!();

        if let (Some((dwarf, fs)), Some(sess)) = (&self.dwarf, &mut self.sess) {
            let mut core = sess.core(0)?;
            use crate::Debugger;
            let mut debugger = Debugger::new(dwarf, fs);
            let stacktrace = debugger.get_current_stacktrace(&mut core)?;

            if let Some(s) = stacktrace.iter().find(|sf| sf.call_frame.id as i64 == args.frame_id) {
                let source = debugserver_types::Source {
                    name: s.source.file.clone(),
                    path: s.source.directory.clone(),
                    source_reference: None,
                    presentation_hint: None,
                    origin: None,
                    sources: None,
                    adapter_data: None,
                    checksums: None,
                };
                scopes.push(debugserver_types::Scope {
                    column: s.source.column.map(|v| v as i64),
                    end_column: None,
                    end_line: None,
                    expensive: false,
                    indexed_variables: None,
                    line: s.source.line.map(|v| v as i64),
                    name: s.name.clone(),
                    named_variables: None,
                    source: Some(source),
                    variables_reference: s.call_frame.id as i64,
                });
            }
        }


        let body = debugserver_types::ScopesResponseBody {
            scopes: scopes,
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


    pub fn variables_command_request(&mut self, req: &Request) -> Result<bool>
    {
        println!("variables here");

        let args: debugserver_types::VariablesArguments = get_arguments(&req)?;
        debug!("args: {:?}", args);

        let mut variables = vec!();

        // TODO: get variables

        let body = debugserver_types::VariablesResponseBody {
            variables: variables,
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
        match self.run_core() {
            Ok(_) => (),
            Err(err) => debug!("Error: {:?}", err),
        };
        
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

        let new_breakpoints = self.update_breakpoints(breakpoints, args.source.path)?;
       
        let body = SetBreakpointsResponseBody {
            breakpoints: new_breakpoints,
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

