use std::ops::Deref;

use rust_dds_api::{
    dcps_psm::{SampleLostStatus, SampleRejectedStatus, SubscriptionMatchedStatus},
    infrastructure::{entity::Entity, qos::DataReaderQos},
    return_type::DDSResult,
    subscription::{data_reader::DataReader, data_reader_listener::DataReaderListener},
    topic::topic_description::TopicDescription,
};
use rust_rtps_pim::{
    behavior::reader::{
        reader::RtpsReader,
        stateless_reader::{RtpsStatelessReader, RtpsStatelessReaderOperations},
    },
    structure::{types::Locator, RtpsEndpoint, RtpsEntity, RtpsHistoryCache},
};

use crate::{
    dds_type::DdsDeserialize, rtps_impl::rtps_reader_history_cache_impl::ReaderHistoryCache,
};

pub enum RtpsReaderFlavor {
    Stateful,
    Stateless(RtpsStatelessReader<Vec<Locator>, ReaderHistoryCache>),
}

impl Deref for RtpsReaderFlavor {
    type Target = RtpsReader<Vec<Locator>, ReaderHistoryCache>;

    fn deref(&self) -> &Self::Target {
        match self {
            RtpsReaderFlavor::Stateful => todo!(),
            RtpsReaderFlavor::Stateless(stateless_reader) => &stateless_reader.0,
        }
    }
}

impl RtpsStatelessReaderOperations for RtpsReaderFlavor {
    fn new(
        guid: rust_rtps_pim::structure::types::Guid,
        topic_kind: rust_rtps_pim::structure::types::TopicKind,
        reliability_level: rust_rtps_pim::structure::types::ReliabilityKind,
        unicast_locator_list: &[Locator],
        multicast_locator_list: &[Locator],
        heartbeat_response_delay: rust_rtps_pim::behavior::types::Duration,
        heartbeat_supression_duration: rust_rtps_pim::behavior::types::Duration,
        expects_inline_qos: bool,
    ) -> Self {
        Self::Stateless(RtpsStatelessReader(RtpsReader {
            endpoint: RtpsEndpoint {
                entity: { RtpsEntity { guid } },
                topic_kind,
                reliability_level,
                unicast_locator_list: unicast_locator_list.into_iter().cloned().collect(),
                multicast_locator_list: multicast_locator_list.into_iter().cloned().collect(),
            },
            heartbeat_response_delay,
            heartbeat_supression_duration,
            reader_cache: ReaderHistoryCache::new(),
            expects_inline_qos,
        }))
    }
}

pub struct DataReaderImpl {
    rtps_reader: RtpsReaderFlavor,
    qos: DataReaderQos,
}

impl DataReaderImpl {
    pub fn new(qos: DataReaderQos, rtps_reader: RtpsReaderFlavor) -> Self {
        Self { rtps_reader, qos }
    }

    pub fn set_qos(&mut self, qos: Option<DataReaderQos>) -> DDSResult<()> {
        self.qos = qos.unwrap_or_default();
        Ok(())
    }

    pub fn get_qos(&self) -> DDSResult<&DataReaderQos> {
        Ok(&self.qos)
    }

    /// Get a mutable reference to the data reader impl's rtps reader.
    pub fn rtps_reader_mut(&mut self) -> &mut RtpsReaderFlavor {
        &mut self.rtps_reader
    }

    /// Get a reference to the data reader impl's rtps reader.
    pub fn rtps_reader(&self) -> &RtpsReaderFlavor {
        &self.rtps_reader
    }
}

// let shared_reader = self.reader.upgrade()?;
// let mut reader = shared_reader.lock();
// let reader_cache = reader.rtps_reader_mut().reader_cache_mut();
// Ok(reader_cache
//     .changes_mut()
//     .iter()
//     .map(|cc| {
//         let data = cc.data();
//         let value = cdr::deserialize(data).unwrap();
//         let sample_info = SampleInfo {
//             sample_state: *cc.sample_state_kind(),
//             view_state: *cc.view_state_kind(),
//             instance_state: *cc.instance_state_kind(),
//             disposed_generation_count: 0,
//             no_writers_generation_count: 0,
//             sample_rank: 0,
//             generation_rank: 0,
//             absolute_generation_rank: 0,
//             source_timestamp: Time { sec: 0, nanosec: 0 },
//             instance_handle: 0,
//             publication_handle: 0,
//             valid_data: true,
//         };
//         (value, sample_info)
//     })
//     .collect())

impl<T> DataReader<T> for DataReaderImpl
where
    T: for<'de> DdsDeserialize<'de>,
{
    type Samples = Vec<T>;

    fn read(
        &self,
        _max_samples: i32,
        _sample_states: &[rust_dds_api::dcps_psm::SampleStateKind],
        _view_states: &[rust_dds_api::dcps_psm::ViewStateKind],
        _instance_states: &[rust_dds_api::dcps_psm::InstanceStateKind],
    ) -> DDSResult<Self::Samples> {
        if let Some(cc) = self.rtps_reader.reader_cache.get_change(&1) {
            let mut data = cc.data_value;
            let result: T = DdsDeserialize::deserialize(&mut data).unwrap();
            Ok(vec![result])
        } else {
            Ok(vec![])
        }
    }

    fn take(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _sample_states: &[rust_dds_api::dcps_psm::SampleStateKind],
        _view_states: &[rust_dds_api::dcps_psm::ViewStateKind],
        _instance_states: &[rust_dds_api::dcps_psm::InstanceStateKind],
    ) -> DDSResult<()> {
        todo!()
    }

    fn read_w_condition(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _a_condition: rust_dds_api::infrastructure::read_condition::ReadCondition,
    ) -> DDSResult<()> {
        todo!()
    }

    fn take_w_condition(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _a_condition: rust_dds_api::infrastructure::read_condition::ReadCondition,
    ) -> DDSResult<()> {
        todo!()
    }

    fn read_next_sample(
        &self,
        _data_value: &mut [T],
        _sample_info: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
    ) -> DDSResult<()> {
        todo!()
    }

    fn take_next_sample(
        &self,
        _data_value: &mut [T],
        _sample_info: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
    ) -> DDSResult<()> {
        todo!()
    }

    fn read_instance(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _a_handle: rust_dds_api::dcps_psm::InstanceHandle,
        _sample_states: &[rust_dds_api::dcps_psm::SampleStateKind],
        _view_states: &[rust_dds_api::dcps_psm::ViewStateKind],
        _instance_states: &[rust_dds_api::dcps_psm::InstanceStateKind],
    ) -> DDSResult<()> {
        todo!()
    }

    fn take_instance(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _a_handle: rust_dds_api::dcps_psm::InstanceHandle,
        _sample_states: &[rust_dds_api::dcps_psm::SampleStateKind],
        _view_states: &[rust_dds_api::dcps_psm::ViewStateKind],
        _instance_states: &[rust_dds_api::dcps_psm::InstanceStateKind],
    ) -> DDSResult<()> {
        todo!()
    }

    fn read_next_instance(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _previous_handle: rust_dds_api::dcps_psm::InstanceHandle,
        _sample_states: &[rust_dds_api::dcps_psm::SampleStateKind],
        _view_states: &[rust_dds_api::dcps_psm::ViewStateKind],
        _instance_states: &[rust_dds_api::dcps_psm::InstanceStateKind],
    ) -> DDSResult<()> {
        todo!()
    }

    fn take_next_instance(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _previous_handle: rust_dds_api::dcps_psm::InstanceHandle,
        _sample_states: &[rust_dds_api::dcps_psm::SampleStateKind],
        _view_states: &[rust_dds_api::dcps_psm::ViewStateKind],
        _instance_states: &[rust_dds_api::dcps_psm::InstanceStateKind],
    ) -> DDSResult<()> {
        todo!()
    }

    fn read_next_instance_w_condition(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _previous_handle: rust_dds_api::dcps_psm::InstanceHandle,
        _a_condition: rust_dds_api::infrastructure::read_condition::ReadCondition,
    ) -> DDSResult<()> {
        todo!()
    }

    fn take_next_instance_w_condition(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
        _max_samples: i32,
        _previous_handle: rust_dds_api::dcps_psm::InstanceHandle,
        _a_condition: rust_dds_api::infrastructure::read_condition::ReadCondition,
    ) -> DDSResult<()> {
        todo!()
    }

    fn return_loan(
        &self,
        _data_values: &mut [T],
        _sample_infos: &mut [rust_dds_api::infrastructure::sample_info::SampleInfo],
    ) -> DDSResult<()> {
        todo!()
    }

    fn get_key_value(
        &self,
        _key_holder: &mut T,
        _handle: rust_dds_api::dcps_psm::InstanceHandle,
    ) -> DDSResult<()> {
        todo!()
    }

    fn lookup_instance(&self, _instance: &T) -> rust_dds_api::dcps_psm::InstanceHandle {
        todo!()
    }

    fn create_readcondition(
        &self,
        _sample_states: &[rust_dds_api::dcps_psm::SampleStateKind],
        _view_states: &[rust_dds_api::dcps_psm::ViewStateKind],
        _instance_states: &[rust_dds_api::dcps_psm::InstanceStateKind],
    ) -> rust_dds_api::infrastructure::read_condition::ReadCondition {
        todo!()
    }

    fn create_querycondition(
        &self,
        _sample_states: &[rust_dds_api::dcps_psm::SampleStateKind],
        _view_states: &[rust_dds_api::dcps_psm::ViewStateKind],
        _instance_states: &[rust_dds_api::dcps_psm::InstanceStateKind],
        _query_expression: &'static str,
        _query_parameters: &[&'static str],
    ) -> rust_dds_api::subscription::query_condition::QueryCondition {
        todo!()
    }

    fn delete_readcondition(
        &self,
        _a_condition: rust_dds_api::infrastructure::read_condition::ReadCondition,
    ) -> DDSResult<()> {
        todo!()
    }

    fn get_liveliness_changed_status(
        &self,
        _status: &mut rust_dds_api::dcps_psm::LivelinessChangedStatus,
    ) -> DDSResult<()> {
        todo!()
    }

    fn get_requested_deadline_missed_status(
        &self,
        _status: &mut rust_dds_api::dcps_psm::RequestedDeadlineMissedStatus,
    ) -> DDSResult<()> {
        todo!()
    }

    fn get_requested_incompatible_qos_status(
        &self,
        _status: &mut rust_dds_api::dcps_psm::RequestedIncompatibleQosStatus,
    ) -> DDSResult<()> {
        todo!()
    }

    fn get_sample_lost_status(&self, _status: &mut SampleLostStatus) -> DDSResult<()> {
        todo!()
    }

    fn get_sample_rejected_status(&self, _status: &mut SampleRejectedStatus) -> DDSResult<()> {
        todo!()
    }

    fn get_subscription_matched_status(
        &self,
        _status: &mut SubscriptionMatchedStatus,
    ) -> DDSResult<()> {
        todo!()
    }

    fn get_topicdescription(&self) -> &dyn TopicDescription<T> {
        todo!()
    }

    fn get_subscriber(&self) -> &dyn rust_dds_api::subscription::subscriber::Subscriber {
        todo!()
    }

    fn delete_contained_entities(&self) -> DDSResult<()> {
        todo!()
    }

    fn wait_for_historical_data(&self) -> DDSResult<()> {
        todo!()
    }

    fn get_matched_publication_data(
        &self,
        _publication_data: &mut rust_dds_api::builtin_topics::PublicationBuiltinTopicData,
        _publication_handle: rust_dds_api::dcps_psm::InstanceHandle,
    ) -> DDSResult<()> {
        todo!()
    }

    fn get_match_publication(
        &self,
        _publication_handles: &mut [rust_dds_api::dcps_psm::InstanceHandle],
    ) -> DDSResult<()> {
        todo!()
    }
}

impl Entity for DataReaderImpl {
    type Qos = DataReaderQos;

    type Listener = &'static dyn DataReaderListener<DataPIM = ()>;

    fn set_qos(&mut self, _qos: Option<Self::Qos>) -> DDSResult<()> {
        todo!()
    }

    fn get_qos(&self) -> DDSResult<Self::Qos> {
        todo!()
    }

    fn set_listener(
        &self,
        _a_listener: Option<Self::Listener>,
        _mask: rust_dds_api::dcps_psm::StatusMask,
    ) -> DDSResult<()> {
        todo!()
    }

    fn get_listener(&self) -> DDSResult<Option<Self::Listener>> {
        todo!()
    }

    fn get_statuscondition(
        &self,
    ) -> DDSResult<rust_dds_api::infrastructure::entity::StatusCondition> {
        todo!()
    }

    fn get_status_changes(&self) -> DDSResult<rust_dds_api::dcps_psm::StatusMask> {
        todo!()
    }

    fn enable(&self) -> DDSResult<()> {
        todo!()
    }

    fn get_instance_handle(&self) -> DDSResult<rust_dds_api::dcps_psm::InstanceHandle> {
        todo!()
    }
}
