use std::sync::{Arc, Mutex};
use crate::behavior::StatefulWriter;

use rust_dds_interface::protocol::{ProtocolEntity, ProtocolWriter};
use rust_dds_interface::types::{InstanceHandle, ChangeKind, Data, ParameterList};
use rust_dds_interface::cache_change::CacheChange;
use rust_dds_interface::history_cache::HistoryCache;

pub struct Writer {
    writer: Arc<StatefulWriter>,
}

impl Writer {
    pub fn new(writer: Arc<StatefulWriter>) -> Self {
        Self {
            writer
        }
    }
}


impl ProtocolEntity for Writer {
    fn get_instance_handle(&self) -> InstanceHandle {
        todo!()
    }
}

impl ProtocolWriter for Writer {
    fn new_change(&self, _kind: ChangeKind, _data: Option<Data>, _inline_qos: Option<ParameterList>, _handle: InstanceHandle) -> CacheChange {
        // self.writer.new_change(kind, data, inline_qos, handle)
        todo!()
    }

    fn writer_cache(&self) -> &Mutex<HistoryCache> {
        // self.writer.writer_cache()
        todo!()
    }
}