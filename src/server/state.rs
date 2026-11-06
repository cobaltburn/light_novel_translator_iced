use crate::{
    error::{Error, Result},
    llm::{Client, Settings},
    server::Method,
};
use iced::{Task, task::Handle};

#[derive(Default, Debug)]
pub struct Server {
    pub client: Client,
    pub models: Vec<String>,
    pub current_model: Option<String>,
    pub handles: Vec<Handle>, // handles must be added with abort on drop
    pub settings: Settings,
    pub method: Method,
}

impl Server {
    pub fn connected(&self) -> bool {
        self.client.connected()
    }

    pub fn bind_handle<T: 'static>(&mut self, task: Task<T>) -> Task<T> {
        bind(&mut self.handles, task)
    }

    pub fn copy(&self) -> Self {
        Self {
            client: self.client.clone(),
            models: self.models.clone(),
            current_model: self.current_model.clone(),
            settings: self.settings.clone(),
            method: self.method,
            handles: Vec::new(),
        }
    }

    pub fn check_ready(&self) -> Result<String> {
        if !self.connected() {
            return Err(Error::ServerError("Not connected to a server"));
        }
        self.current_model
            .clone()
            .ok_or(Error::ServerError("No model selected"))
    }
}

pub(super) fn bind<T: 'static>(handles: &mut Vec<Handle>, task: Task<T>) -> Task<T> {
    let (task, handle) = task.abortable();
    handles.push(handle.abort_on_drop());
    task
}
