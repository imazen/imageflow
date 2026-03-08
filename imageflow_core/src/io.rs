//! I/O proxy — manages input/output buffers for the pipeline.

use crate::error::FlowError;
use std::collections::HashMap;
use std::sync::Arc;

pub use imageflow_types::IoDirection;

/// Manages I/O buffers referenced by io_id in the pipeline.
#[derive(Default)]
pub struct IoStore {
    objects: HashMap<i32, IoProxy>,
}

impl IoStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an input buffer.
    pub fn add_input(&mut self, io_id: i32, data: Arc<[u8]>) {
        self.objects.insert(
            io_id,
            IoProxy {
                direction: IoDirection::In,
                data: IoData::Input(data),
            },
        );
    }

    /// Add an output buffer slot.
    pub fn add_output(&mut self, io_id: i32) {
        self.objects.insert(
            io_id,
            IoProxy {
                direction: IoDirection::Out,
                data: IoData::Output(Vec::new()),
            },
        );
    }

    /// Get an input buffer by io_id.
    pub fn get_input(&self, io_id: i32) -> Result<&[u8], FlowError> {
        match self.objects.get(&io_id) {
            Some(IoProxy {
                data: IoData::Input(data),
                ..
            }) => Ok(data),
            Some(_) => Err(FlowError::InvalidPipeline(format!(
                "io_id {io_id} is not an input"
            ))),
            None => Err(FlowError::IoNotFound(io_id)),
        }
    }

    /// Write to an output buffer.
    pub fn write_output(&mut self, io_id: i32, data: Vec<u8>) -> Result<(), FlowError> {
        match self.objects.get_mut(&io_id) {
            Some(IoProxy {
                data: IoData::Output(buf),
                ..
            }) => {
                *buf = data;
                Ok(())
            }
            Some(_) => Err(FlowError::InvalidPipeline(format!(
                "io_id {io_id} is not an output"
            ))),
            None => Err(FlowError::IoNotFound(io_id)),
        }
    }

    /// Get the output buffer by io_id.
    pub fn get_output(&self, io_id: i32) -> Result<&[u8], FlowError> {
        match self.objects.get(&io_id) {
            Some(IoProxy {
                data: IoData::Output(buf),
                ..
            }) => Ok(buf),
            Some(_) => Err(FlowError::InvalidPipeline(format!(
                "io_id {io_id} is not an output"
            ))),
            None => Err(FlowError::IoNotFound(io_id)),
        }
    }

    /// Take ownership of the output buffer.
    pub fn take_output(&mut self, io_id: i32) -> Result<Vec<u8>, FlowError> {
        match self.objects.get_mut(&io_id) {
            Some(IoProxy {
                data: IoData::Output(buf),
                ..
            }) => Ok(std::mem::take(buf)),
            Some(_) => Err(FlowError::InvalidPipeline(format!(
                "io_id {io_id} is not an output"
            ))),
            None => Err(FlowError::IoNotFound(io_id)),
        }
    }
}

pub struct IoProxy {
    pub direction: IoDirection,
    pub data: IoData,
}

pub enum IoData {
    Input(Arc<[u8]>),
    Output(Vec<u8>),
}
