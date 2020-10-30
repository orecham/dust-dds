use std::sync::{Arc, Mutex};

use rust_dds_interface::types::{ReturnCode, InstanceHandle, TopicKind};
use rust_dds_interface::protocol::{ProtocolEntity, ProtocolPublisher, ProtocolSubscriber, ProtocolWriter, ProtocolReader};
use rust_dds_interface::qos::{DataWriterQos, DataReaderQos};

use crate::types::GUID;
use crate::structure::{RtpsEndpoint, RtpsEntity, };

pub struct RtpsGroup {
    guid: GUID,
    endpoints: Vec<Arc<Mutex<dyn RtpsEndpoint>>>,
}

impl RtpsGroup {
    pub fn new(guid: GUID) -> Self {
        Self {
            guid,
            endpoints: Vec::new(),
        }
    }

    pub fn mut_endpoints(&mut self) -> &mut Vec<Arc<Mutex<dyn RtpsEndpoint>>> {
        &mut self.endpoints
    }

    pub fn endpoints(&self) -> &[Arc<Mutex<dyn RtpsEndpoint>>] {
        self.endpoints.as_slice()
    }
}

impl RtpsEntity for RtpsGroup {
    fn guid(&self) -> GUID {
        self.guid
    }
}

impl ProtocolEntity for RtpsGroup {
    fn enable(&self) -> ReturnCode<()> {
        todo!()
    }

    fn get_instance_handle(&self) -> InstanceHandle {
        self.guid.into()
    }
}

impl ProtocolPublisher for RtpsGroup {
    fn create_writer(&mut self, _topic_kind: TopicKind, _data_writer_qos: &DataWriterQos) -> Arc<Mutex<dyn ProtocolWriter>> {
        todo!()
    }
}

impl ProtocolSubscriber for RtpsGroup {
    fn create_reader(&mut self, _topic_kind: TopicKind, _data_reader_qos: &DataReaderQos) -> Arc<Mutex<dyn ProtocolReader>> {
        todo!()
    }
}