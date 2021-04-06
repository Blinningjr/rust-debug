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
};

use log::{
    debug,
    error,
    info,
    trace,
    warn,
};


use super::{
    read_dwarf,
    attach_probe,
};

use std::path::PathBuf;

impl<R: Read, W: Write> Session<R, W> {
    pub fn launch_command_request(&mut self, req: Request) -> Result<()> 
    {
        // TODO start the debugee

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
    
        Ok(())
    }


    pub fn attach_command_request(&mut self, req: Request) -> Result<()> 
    {
        let args: AttachRequestArguments = get_arguments(&req)?;
        debug!("> program: {:?}", args.program);
        self.file_path = Some(PathBuf::from(args.program));

        self.dwarf = Some(read_dwarf(self.file_path.as_ref().unwrap())?);
        debug!("> Readed dwarf file");

        match attach_probe() {
            Ok(s) => {
                debug!("> probe attached");
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

                return Ok(());
            },
            Err(e) => {
                warn!("> probe failed to attach");
                return Err(e);
            },
        };
    }


    pub fn threads_command_request(&mut self, req: Request) -> Result<()> 
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

        Ok(())
    }


    pub fn configuration_done_command_request(&mut self, req: Request) -> Result<()>
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
    
        Ok(())
    }


    pub fn pause_command_request(&mut self, req: Request) -> Result<()>
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

        // TODO: Paus program
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
 
        Ok(())
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

