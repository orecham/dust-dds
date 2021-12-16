use rust_rtps_pim::{
    behavior::{
        types::Duration,
        writer::{
            reader_proxy::RtpsReaderProxy,
            stateful_writer::{RtpsStatefulWriterConstructor, RtpsStatefulWriterOperations},
            writer::{RtpsWriterAttributes, RtpsWriterOperations},
        },
    },
    messages::submessage_elements::Parameter,
    structure::{
        cache_change::RtpsCacheChange,
        endpoint::RtpsEndpointAttributes,
        entity::RtpsEntityAttributes,
        history_cache::{RtpsHistoryCacheAddChange, RtpsHistoryCacheConstructor},
        types::{
            ChangeKind, Guid, InstanceHandle, Locator, ReliabilityKind, SequenceNumber, TopicKind,
        },
    },
};

use crate::dds_type::DdsSerialize;

use super::{
    rtps_reader_proxy_impl::RtpsReaderProxyImpl,
    rtps_writer_history_cache_impl::{WriterHistoryCache, WriterHistoryCacheAddChangeMut},
};

pub struct RtpsStatefulWriterImpl {
    pub guid: Guid,
    pub topic_kind: TopicKind,
    pub reliability_level: ReliabilityKind,
    pub unicast_locator_list: Vec<Locator>,
    pub multicast_locator_list: Vec<Locator>,
    pub push_mode: bool,
    pub heartbeat_period: Duration,
    pub nack_response_delay: Duration,
    pub nack_suppression_duration: Duration,
    pub last_change_sequence_number: SequenceNumber,
    pub data_max_size_serialized: Option<i32>,
    pub writer_cache: WriterHistoryCache,
    pub matched_readers: Vec<RtpsReaderProxyImpl>,
}

impl RtpsStatefulWriterOperations<Vec<Locator>> for RtpsStatefulWriterImpl {
    type ReaderProxyType = RtpsReaderProxyImpl;

    fn matched_reader_add(&mut self, a_reader_proxy: RtpsReaderProxy<Vec<Locator>>) {
        let reader_proxy = RtpsReaderProxyImpl::new(a_reader_proxy);
        self.matched_readers.push(reader_proxy)
    }

    fn matched_reader_remove(&mut self, reader_proxy_guid: &Guid) {
        self.matched_readers
            .retain(|x| &x.remote_reader_guid != reader_proxy_guid);
    }

    fn matched_reader_lookup(&self, a_reader_guid: &Guid) -> Option<&Self::ReaderProxyType> {
        self.matched_readers
            .iter()
            .find(|&x| &x.remote_reader_guid == a_reader_guid)
    }

    fn is_acked_by_all(&self) -> bool {
        todo!()
    }
}
impl RtpsStatefulWriterConstructor for RtpsStatefulWriterImpl {
    fn new(
        guid: Guid,
        topic_kind: TopicKind,
        reliability_level: ReliabilityKind,
        unicast_locator_list: &[Locator],
        multicast_locator_list: &[Locator],
        push_mode: bool,
        heartbeat_period: Duration,
        nack_response_delay: Duration,
        nack_suppression_duration: Duration,
        data_max_size_serialized: Option<i32>,
    ) -> Self {
        Self {
            guid,
            topic_kind,
            reliability_level,
            unicast_locator_list: unicast_locator_list.iter().cloned().collect(),
            multicast_locator_list: multicast_locator_list.iter().cloned().collect(),
            push_mode,
            heartbeat_period,
            nack_response_delay,
            nack_suppression_duration,
            last_change_sequence_number: 0,
            data_max_size_serialized,
            writer_cache: WriterHistoryCache::new(),
            matched_readers: Vec::new(),
        }
    }
}

impl RtpsEntityAttributes for RtpsStatefulWriterImpl {
    fn guid(&self) -> &Guid {
        &self.guid
    }
}

impl RtpsEndpointAttributes for RtpsStatefulWriterImpl {
    fn topic_kind(&self) -> &TopicKind {
        &self.topic_kind
    }

    fn reliability_level(&self) -> &ReliabilityKind {
        &self.reliability_level
    }

    fn unicast_locator_list(&self) -> &[Locator] {
        self.unicast_locator_list.as_ref()
    }

    fn multicast_locator_list(&self) -> &[Locator] {
        self.multicast_locator_list.as_ref()
    }
}

impl RtpsWriterAttributes for RtpsStatefulWriterImpl {
    type WriterHistoryCacheType = WriterHistoryCache;

    fn push_mode(&self) -> &bool {
        &self.push_mode
    }

    fn heartbeat_period(&self) -> &Duration {
        &self.heartbeat_period
    }

    fn nack_response_delay(&self) -> &Duration {
        &self.nack_response_delay
    }

    fn nack_suppression_duration(&self) -> &Duration {
        &self.nack_suppression_duration
    }

    fn last_change_sequence_number(&self) -> &SequenceNumber {
        &self.last_change_sequence_number
    }

    fn data_max_size_serialized(&self) -> &Option<i32> {
        &self.data_max_size_serialized
    }

    fn writer_cache(&self) -> &Self::WriterHistoryCacheType {
        &self.writer_cache
    }
}

impl RtpsWriterOperations for RtpsStatefulWriterImpl {
    fn new_change<'a, P, D>(
        &mut self,
        kind: ChangeKind,
        data: D,
        inline_qos: P,
        handle: InstanceHandle,
    ) -> RtpsCacheChange<P, D> {
        self.last_change_sequence_number = self.last_change_sequence_number + 1;
        RtpsCacheChange {
            kind,
            writer_guid: self.guid,
            instance_handle: handle,
            sequence_number: self.last_change_sequence_number,
            data_value: data,
            inline_qos,
        }
    }
}

impl<T> WriterHistoryCacheAddChangeMut<'_, T> for RtpsStatefulWriterImpl
where
    T: DdsSerialize,
{
    fn get_writer_history_cache_add_change_mut(
        &'_ mut self,
    ) -> &mut dyn RtpsHistoryCacheAddChange<Vec<Parameter<Vec<u8>>>, &'_ T> {
        &mut self.writer_cache
    }
}
