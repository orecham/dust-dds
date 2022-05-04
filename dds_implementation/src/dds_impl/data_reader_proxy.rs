use super::{
    subscriber_proxy::{SubscriberAttributes, SubscriberProxy},
    topic_proxy::{TopicAttributes, TopicProxy},
};
use crate::{
    dds_type::DdsDeserialize,
    utils::{
        rtps_structure::RtpsStructure,
        shared_object::{DdsRwLock, DdsShared, DdsWeak},
        timer::Timer,
    },
};
use dds_api::{
    builtin_topics::PublicationBuiltinTopicData,
    dcps_psm::{
        InstanceHandle, InstanceStateMask, LivelinessChangedStatus, RequestedDeadlineMissedStatus,
        RequestedIncompatibleQosStatus, SampleLostStatus, SampleRejectedStatus, SampleStateMask,
        StatusMask, SubscriptionMatchedStatus, Time, ViewStateMask, ALIVE_INSTANCE_STATE,
        DATA_AVAILABLE_STATUS, LIVELINESS_CHANGED_STATUS, NEW_VIEW_STATE, NOT_READ_SAMPLE_STATE,
        READ_SAMPLE_STATE, REQUESTED_DEADLINE_MISSED_STATUS, REQUESTED_INCOMPATIBLE_QOS_STATUS,
        SAMPLE_LOST_STATUS, SAMPLE_REJECTED_STATUS, SUBSCRIPTION_MATCHED_STATUS,
    },
    infrastructure::{
        entity::{Entity, StatusCondition},
        qos::DataReaderQos,
        qos_policy::HistoryQosPolicyKind,
        read_condition::ReadCondition,
        sample_info::SampleInfo,
    },
    return_type::{DdsError, DdsResult},
    subscription::{
        data_reader::{DataReader, DataReaderGetSubscriber, DataReaderGetTopicDescription},
        data_reader_listener::DataReaderListener,
        query_condition::QueryCondition,
    },
};
use rtps_pim::{
    behavior::reader::reader::RtpsReaderAttributes,
    structure::{
        cache_change::RtpsCacheChangeAttributes,
        history_cache::{RtpsHistoryCacheAttributes, RtpsHistoryCacheOperations},
        types::SequenceNumber,
    },
};
use std::{collections::HashSet, marker::PhantomData, time::Duration};

pub trait AnyDataReaderListener<Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    fn trigger_on_data_available(&self, reader: DdsShared<DataReaderAttributes<Rtps, T>>);
    fn trigger_on_sample_rejected(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: SampleRejectedStatus,
    );
    fn trigger_on_liveliness_changed(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: LivelinessChangedStatus,
    );
    fn trigger_on_requested_deadline_missed(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: RequestedDeadlineMissedStatus,
    );
    fn trigger_on_requested_incompatible_qos(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: RequestedIncompatibleQosStatus,
    );
    fn trigger_on_subscription_matched(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: SubscriptionMatchedStatus,
    );
    fn trigger_on_sample_lost(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: SampleLostStatus,
    );
}

impl<Foo, Rtps, T> AnyDataReaderListener<Rtps, T>
    for Box<dyn DataReaderListener<Foo = Foo> + Send + Sync>
where
    Foo: for<'de> DdsDeserialize<'de> + 'static,
    Rtps: RtpsStructure,
    T: Timer,
{
    fn trigger_on_data_available(&self, reader: DdsShared<DataReaderAttributes<Rtps, T>>) {
        *reader.status_change.write_lock() &= !DATA_AVAILABLE_STATUS;
        let data_reader = DataReaderProxy::new(reader.downgrade());
        self.on_data_available(&data_reader)
    }

    fn trigger_on_sample_rejected(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: SampleRejectedStatus,
    ) {
        *reader.status_change.write_lock() &= !SAMPLE_REJECTED_STATUS;
        let data_reader = DataReaderProxy::new(reader.downgrade());
        self.on_sample_rejected(&data_reader, status)
    }

    fn trigger_on_liveliness_changed(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: LivelinessChangedStatus,
    ) {
        *reader.status_change.write_lock() &= !LIVELINESS_CHANGED_STATUS;
        let data_reader = DataReaderProxy::new(reader.downgrade());
        self.on_liveliness_changed(&data_reader, status)
    }

    fn trigger_on_requested_deadline_missed(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: RequestedDeadlineMissedStatus,
    ) {
        *reader.status_change.write_lock() &= !REQUESTED_DEADLINE_MISSED_STATUS;
        let data_reader = DataReaderProxy::new(reader.downgrade());
        self.on_requested_deadline_missed(&data_reader, status)
    }

    fn trigger_on_requested_incompatible_qos(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: RequestedIncompatibleQosStatus,
    ) {
        *reader.status_change.write_lock() &= !REQUESTED_INCOMPATIBLE_QOS_STATUS;
        let data_reader = DataReaderProxy::new(reader.downgrade());
        self.on_requested_incompatible_qos(&data_reader, status)
    }

    fn trigger_on_subscription_matched(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: SubscriptionMatchedStatus,
    ) {
        *reader.status_change.write_lock() &= !SUBSCRIPTION_MATCHED_STATUS;
        let data_reader = DataReaderProxy::new(reader.downgrade());
        self.on_subscription_matched(&data_reader, status)
    }

    fn trigger_on_sample_lost(
        &self,
        reader: DdsShared<DataReaderAttributes<Rtps, T>>,
        status: SampleLostStatus,
    ) {
        *reader.status_change.write_lock() &= !SAMPLE_LOST_STATUS;
        let data_reader = DataReaderProxy::new(reader.downgrade());
        self.on_sample_lost(&data_reader, status)
    }
}

pub enum RtpsReader<Rtps>
where
    Rtps: RtpsStructure,
{
    Stateless(Rtps::StatelessReader),
    Stateful(Rtps::StatefulReader),
}

impl<Rtps: RtpsStructure> RtpsReader<Rtps> {
    pub fn try_as_stateless_reader(&mut self) -> DdsResult<&mut Rtps::StatelessReader> {
        match self {
            RtpsReader::Stateless(x) => Ok(x),
            RtpsReader::Stateful(_) => Err(DdsError::PreconditionNotMet(
                "Not a stateless reader".to_string(),
            )),
        }
    }

    pub fn try_as_stateful_reader(&mut self) -> DdsResult<&mut Rtps::StatefulReader> {
        match self {
            RtpsReader::Stateless(_) => Err(DdsError::PreconditionNotMet(
                "Not a stateful reader".to_string(),
            )),
            RtpsReader::Stateful(x) => Ok(x),
        }
    }
}

impl<Rtps: RtpsStructure> RtpsReaderAttributes for RtpsReader<Rtps> {
    type HistoryCacheType = Rtps::HistoryCache;

    fn heartbeat_response_delay(&self) -> rtps_pim::behavior::types::Duration {
        match self {
            RtpsReader::Stateless(reader) => reader.heartbeat_response_delay(),
            RtpsReader::Stateful(reader) => reader.heartbeat_response_delay(),
        }
    }

    fn heartbeat_suppression_duration(&self) -> rtps_pim::behavior::types::Duration {
        match self {
            RtpsReader::Stateless(reader) => reader.heartbeat_suppression_duration(),
            RtpsReader::Stateful(reader) => reader.heartbeat_suppression_duration(),
        }
    }

    fn reader_cache(&mut self) -> &mut Self::HistoryCacheType {
        match self {
            RtpsReader::Stateless(reader) => reader.reader_cache(),
            RtpsReader::Stateful(reader) => reader.reader_cache(),
        }
    }

    fn expects_inline_qos(&self) -> bool {
        match self {
            RtpsReader::Stateless(reader) => reader.expects_inline_qos(),
            RtpsReader::Stateful(reader) => reader.expects_inline_qos(),
        }
    }
}

pub struct DataReaderAttributes<Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    pub rtps_reader: DdsRwLock<RtpsReader<Rtps>>,
    pub qos: DataReaderQos,
    pub topic: DdsShared<TopicAttributes<Rtps>>,
    pub listener: DdsRwLock<Option<Box<dyn AnyDataReaderListener<Rtps, T> + Send + Sync>>>,
    pub parent_subscriber: DdsWeak<SubscriberAttributes<Rtps>>,
    pub status: DdsRwLock<SubscriptionMatchedStatus>,
    pub samples_read: DdsRwLock<HashSet<SequenceNumber>>,
    pub deadline_timer: DdsRwLock<T>,
    pub requested_deadline_missed_status: DdsRwLock<RequestedDeadlineMissedStatus>,
    pub status_change: DdsRwLock<StatusMask>,
}

impl<Rtps, T> DataReaderAttributes<Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    pub fn new(
        qos: DataReaderQos,
        rtps_reader: RtpsReader<Rtps>,
        topic: DdsShared<TopicAttributes<Rtps>>,
        listener: Option<Box<dyn AnyDataReaderListener<Rtps, T> + Send + Sync>>,
        parent_subscriber: DdsWeak<SubscriberAttributes<Rtps>>,
    ) -> Self {
        let deadline_duration = Duration::from_secs(*qos.deadline.period.sec() as u64)
            + Duration::from_nanos(*qos.deadline.period.nanosec() as u64);

        Self {
            rtps_reader: DdsRwLock::new(rtps_reader),
            qos: qos,
            topic,
            listener: DdsRwLock::new(listener),
            parent_subscriber,
            status: DdsRwLock::new(SubscriptionMatchedStatus {
                total_count: 0,
                total_count_change: 0,
                last_publication_handle: 0,
                current_count: 0,
                current_count_change: 0,
            }),
            samples_read: DdsRwLock::new(HashSet::new()),
            deadline_timer: DdsRwLock::new(T::new(deadline_duration)),
            requested_deadline_missed_status: DdsRwLock::new(RequestedDeadlineMissedStatus {
                total_count: 0,
                total_count_change: 0,
                last_instance_handle: 0,
            }),
            status_change: DdsRwLock::new(0),
        }
    }

    pub fn read_sample<'a>(&self, cache_change: &'a Rtps::CacheChange) -> (&'a [u8], SampleInfo) {
        *self.status_change.write_lock() &= !DATA_AVAILABLE_STATUS;

        let mut samples_read = self.samples_read.write_lock();
        let data_value = cache_change.data_value();

        let sample_state = {
            let sn = cache_change.sequence_number();
            if samples_read.contains(&sn) {
                READ_SAMPLE_STATE
            } else {
                samples_read.insert(sn);
                NOT_READ_SAMPLE_STATE
            }
        };

        let sample_info = SampleInfo {
            sample_state,
            view_state: NEW_VIEW_STATE,
            instance_state: ALIVE_INSTANCE_STATE,
            disposed_generation_count: 0,
            no_writers_generation_count: 0,
            sample_rank: 0,
            generation_rank: 0,
            absolute_generation_rank: 0,
            source_timestamp: Time { sec: 0, nanosec: 0 },
            instance_handle: 0,
            publication_handle: 0,
            valid_data: true,
        };

        (data_value, sample_info)
    }
}

impl<Rtps, T> DataReaderAttributes<Rtps, T>
where
    T: Timer + Send + Sync + 'static,
    Rtps: RtpsStructure + 'static,
    Rtps::Group: Send + Sync,
    Rtps::Participant: Send + Sync,

    Rtps::StatelessWriter: Send + Sync,
    Rtps::StatefulWriter: Send + Sync,

    Rtps::StatelessReader: Send + Sync,
    Rtps::StatefulReader: Send + Sync,
    Rtps::HistoryCache: Send + Sync,
    Rtps::CacheChange: Send + Sync,
{
    pub fn on_data_received(reader: DdsShared<Self>) -> DdsResult<()> {
        if reader.qos.history.kind == HistoryQosPolicyKind::KeepLastHistoryQoS {
            let mut rtps_reader = reader.rtps_reader.write_lock();

            let cache_len = rtps_reader.reader_cache().changes().len() as i32;
            if cache_len > reader.qos.history.depth {
                let mut seq_nums: Vec<_> = rtps_reader
                    .reader_cache()
                    .changes()
                    .iter()
                    .map(|c| c.sequence_number())
                    .collect();
                seq_nums.sort();

                let to_delete =
                    &seq_nums[0..(cache_len as usize - reader.qos.history.depth as usize)];
                rtps_reader
                    .reader_cache()
                    .remove_change(|c| to_delete.contains(&c.sequence_number()));
            }
        }

        let reader_shared = reader.clone();
        reader.deadline_timer.write_lock().on_deadline(move || {
            reader_shared
                .requested_deadline_missed_status
                .write_lock()
                .total_count += 1;
            reader_shared
                .requested_deadline_missed_status
                .write_lock()
                .total_count_change += 1;

            *reader_shared.status_change.write_lock() |= REQUESTED_DEADLINE_MISSED_STATUS;
            reader_shared.listener.read_lock().as_ref().map(|l| {
                l.trigger_on_requested_deadline_missed(
                    reader_shared.clone(),
                    reader_shared
                        .requested_deadline_missed_status
                        .read_lock()
                        .clone(),
                )
            });
        });

        *reader.status_change.write_lock() |= DATA_AVAILABLE_STATUS;
        reader
            .listener
            .read_lock()
            .as_ref()
            .map(|l| l.trigger_on_data_available(reader.clone()));

        Ok(())
    }
}

pub struct DataReaderProxy<Foo, Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    data_reader_impl: DdsWeak<DataReaderAttributes<Rtps, T>>,
    phantom: PhantomData<Foo>,
}

// Not automatically derived because in that case it is only available if Foo: Clone
impl<Foo, Rtps, T> Clone for DataReaderProxy<Foo, Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    fn clone(&self) -> Self {
        Self {
            data_reader_impl: self.data_reader_impl.clone(),
            phantom: self.phantom.clone(),
        }
    }
}

impl<Foo, Rtps, T> DataReaderProxy<Foo, Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    pub fn new(data_reader_impl: DdsWeak<DataReaderAttributes<Rtps, T>>) -> Self {
        Self {
            data_reader_impl,
            phantom: PhantomData,
        }
    }
}

impl<Foo, Rtps, T> AsRef<DdsWeak<DataReaderAttributes<Rtps, T>>> for DataReaderProxy<Foo, Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    fn as_ref(&self) -> &DdsWeak<DataReaderAttributes<Rtps, T>> {
        &self.data_reader_impl
    }
}

impl<Foo, Rtps, T> DataReaderGetSubscriber for DataReaderProxy<Foo, Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    type Subscriber = SubscriberProxy<Rtps>;

    fn data_reader_get_subscriber(&self) -> DdsResult<Self::Subscriber> {
        todo!()
    }
}

impl<Foo, Rtps, T> DataReaderGetTopicDescription for DataReaderProxy<Foo, Rtps, T>
where
    Rtps: RtpsStructure,
    T: Timer,
{
    type TopicDescription = TopicProxy<Foo, Rtps>;

    fn data_reader_get_topicdescription(&self) -> DdsResult<Self::TopicDescription> {
        todo!()
    }
}

impl<Foo, Rtps, T> DataReader<Foo> for DataReaderProxy<Foo, Rtps, T>
where
    Foo: for<'de> DdsDeserialize<'de> + 'static,
    Rtps: RtpsStructure,
    T: Timer,
{
    fn read(
        &self,
        max_samples: i32,
        sample_states: SampleStateMask,
        _view_states: ViewStateMask,
        _instance_states: InstanceStateMask,
    ) -> DdsResult<Vec<(Foo, SampleInfo)>> {
        let data_reader_shared = self.data_reader_impl.upgrade()?;
        let mut rtps_reader = data_reader_shared.rtps_reader.write_lock();

        let samples = rtps_reader
            .reader_cache()
            .changes()
            .iter()
            .map(|sample| {
                let (mut data_value, sample_info) = data_reader_shared.read_sample(sample);
                let foo = DdsDeserialize::deserialize(&mut data_value)?;
                Ok((foo, sample_info))
            })
            .filter(|result| {
                if let Ok((_, info)) = result {
                    info.sample_state & sample_states != 0
                } else {
                    true
                }
            })
            .take(max_samples as usize)
            .collect::<DdsResult<Vec<_>>>()?;

        if samples.is_empty() {
            Err(DdsError::NoData)
        } else {
            Ok(samples)
        }
    }

    fn take(
        &self,
        _max_samples: i32,
        sample_states: SampleStateMask,
        _view_states: ViewStateMask,
        _instance_states: InstanceStateMask,
    ) -> DdsResult<Vec<(Foo, SampleInfo)>> {
        let data_reader_shared = self.data_reader_impl.upgrade()?;
        let mut rtps_reader = data_reader_shared.rtps_reader.write_lock();

        let (samples, to_delete): (Vec<_>, Vec<_>) = rtps_reader
            .reader_cache()
            .changes()
            .iter()
            .map(|sample| {
                let (mut data_value, sample_info) = data_reader_shared.read_sample(sample);
                let foo = DdsDeserialize::deserialize(&mut data_value)?;

                Ok(((foo, sample_info), sample.sequence_number()))
            })
            .filter(|result| {
                if let Ok(((_, info), _)) = result {
                    info.sample_state & sample_states != 0
                } else {
                    true
                }
            })
            .collect::<DdsResult<Vec<_>>>()?
            .into_iter()
            .unzip();

        rtps_reader
            .reader_cache()
            .remove_change(|x| to_delete.contains(&x.sequence_number()));

        Ok(samples)
    }

    fn read_w_condition(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
        _max_samples: i32,
        _a_condition: ReadCondition,
    ) -> DdsResult<()> {
        todo!()
    }

    fn take_w_condition(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
        _max_samples: i32,
        _a_condition: ReadCondition,
    ) -> DdsResult<()> {
        todo!()
    }

    fn read_next_sample(
        &self,
        _data_value: &mut [Foo],
        _sample_info: &mut [SampleInfo],
    ) -> DdsResult<()> {
        todo!()
    }

    fn take_next_sample(
        &self,
        _data_value: &mut [Foo],
        _sample_info: &mut [SampleInfo],
    ) -> DdsResult<()> {
        todo!()
    }

    fn read_instance(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
        _max_samples: i32,
        _a_handle: InstanceHandle,
        _sample_states: SampleStateMask,
        _view_states: ViewStateMask,
        _instance_states: InstanceStateMask,
    ) -> DdsResult<()> {
        todo!()
    }

    fn take_instance(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
        _max_samples: i32,
        _a_handle: InstanceHandle,
        _sample_states: SampleStateMask,
        _view_states: ViewStateMask,
        _instance_states: InstanceStateMask,
    ) -> DdsResult<()> {
        todo!()
    }

    fn read_next_instance(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
        _max_samples: i32,
        _previous_handle: InstanceHandle,
        _sample_states: SampleStateMask,
        _view_states: ViewStateMask,
        _instance_states: InstanceStateMask,
    ) -> DdsResult<()> {
        todo!()
    }

    fn take_next_instance(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
        _max_samples: i32,
        _previous_handle: InstanceHandle,
        _sample_states: SampleStateMask,
        _view_states: ViewStateMask,
        _instance_states: InstanceStateMask,
    ) -> DdsResult<()> {
        todo!()
    }

    fn read_next_instance_w_condition(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
        _max_samples: i32,
        _previous_handle: InstanceHandle,
        _a_condition: ReadCondition,
    ) -> DdsResult<()> {
        todo!()
    }

    fn take_next_instance_w_condition(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
        _max_samples: i32,
        _previous_handle: InstanceHandle,
        _a_condition: ReadCondition,
    ) -> DdsResult<()> {
        todo!()
    }

    fn return_loan(
        &self,
        _data_values: &mut [Foo],
        _sample_infos: &mut [SampleInfo],
    ) -> DdsResult<()> {
        todo!()
    }

    fn get_key_value(&self, _key_holder: &mut Foo, _handle: InstanceHandle) -> DdsResult<()> {
        todo!()
    }

    fn lookup_instance(&self, _instance: &Foo) -> DdsResult<InstanceHandle> {
        todo!()
    }

    fn create_readcondition(
        &self,
        _sample_states: SampleStateMask,
        _view_states: ViewStateMask,
        _instance_states: InstanceStateMask,
    ) -> DdsResult<ReadCondition> {
        todo!()
    }

    fn create_querycondition(
        &self,
        _sample_states: SampleStateMask,
        _view_states: ViewStateMask,
        _instance_states: InstanceStateMask,
        _query_expression: &'static str,
        _query_parameters: &[&'static str],
    ) -> DdsResult<QueryCondition> {
        todo!()
    }

    fn delete_readcondition(&self, _a_condition: ReadCondition) -> DdsResult<()> {
        todo!()
    }

    fn get_liveliness_changed_status(
        &self,
        _status: &mut LivelinessChangedStatus,
    ) -> DdsResult<()> {
        todo!()
    }

    fn get_requested_deadline_missed_status(&self) -> DdsResult<RequestedDeadlineMissedStatus> {
        let reader = self.as_ref().upgrade()?;
        let status = reader.requested_deadline_missed_status.read_lock().clone();

        reader
            .requested_deadline_missed_status
            .write_lock()
            .total_count_change = 0;

        Ok(status)
    }

    fn get_requested_incompatible_qos_status(
        &self,
        _status: &mut RequestedIncompatibleQosStatus,
    ) -> DdsResult<()> {
        todo!()
    }

    fn get_sample_lost_status(&self, _status: &mut SampleLostStatus) -> DdsResult<()> {
        todo!()
    }

    fn get_sample_rejected_status(&self, _status: &mut SampleRejectedStatus) -> DdsResult<()> {
        todo!()
    }

    fn get_subscription_matched_status(
        &self,
        _status: &mut SubscriptionMatchedStatus,
    ) -> DdsResult<()> {
        todo!()
    }

    fn delete_contained_entities(&self) -> DdsResult<()> {
        todo!()
    }

    fn wait_for_historical_data(&self) -> DdsResult<()> {
        todo!()
    }

    fn get_matched_publication_data(
        &self,
        _publication_data: &mut PublicationBuiltinTopicData,
        _publication_handle: InstanceHandle,
    ) -> DdsResult<()> {
        todo!()
    }

    fn get_match_publication(&self, _publication_handles: &mut [InstanceHandle]) -> DdsResult<()> {
        todo!()
    }
}

impl<Foo, Rtps, T> Entity for DataReaderProxy<Foo, Rtps, T>
where
    Foo: for<'de> DdsDeserialize<'de> + 'static,
    Rtps: RtpsStructure,
    T: Timer,
{
    type Qos = DataReaderQos;
    type Listener = Box<dyn DataReaderListener<Foo = Foo> + Send + Sync>;

    fn set_qos(&self, _qos: Option<Self::Qos>) -> DdsResult<()> {
        // rtps_shared_write_lock(&rtps_weak_upgrade(&self.data_reader_impl)?).set_qos(qos)
        todo!()
    }

    fn get_qos(&self) -> DdsResult<Self::Qos> {
        // rtps_shared_read_lock(&rtps_weak_upgrade(&self.data_reader_impl)?).get_qos()
        todo!()
    }

    fn set_listener(&self, listener: Option<Self::Listener>, _mask: StatusMask) -> DdsResult<()> {
        *self.as_ref().upgrade()?.listener.write_lock() = match listener {
            Some(l) => Some(Box::new(l)),
            None => None,
        };
        Ok(())
    }

    fn get_listener(&self) -> DdsResult<Option<Self::Listener>> {
        // rtps_shared_read_lock(&rtps_weak_upgrade(&self.data_reader_impl)?).get_listener()
        todo!()
    }

    fn get_statuscondition(&self) -> DdsResult<StatusCondition> {
        // rtps_shared_read_lock(&rtps_weak_upgrade(&self.data_reader_impl)?).get_statuscondition()
        todo!()
    }

    fn get_status_changes(&self) -> DdsResult<StatusMask> {
        Ok(*self.as_ref().upgrade()?.status_change.read_lock())
    }

    fn enable(&self) -> DdsResult<()> {
        // rtps_shared_read_lock(&rtps_weak_upgrade(&self.data_reader_impl)?).enable()
        todo!()
    }

    fn get_instance_handle(&self) -> DdsResult<InstanceHandle> {
        // rtps_shared_read_lock(&rtps_weak_upgrade(&self.data_reader_impl)?).get_instance_handle()
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        dds_impl::topic_proxy::TopicAttributes,
        dds_type::{DdsSerialize, DdsType, Endianness},
        test_utils::{
            mock_rtps::MockRtps, mock_rtps_cache_change::MockRtpsCacheChange,
            mock_rtps_history_cache::MockRtpsHistoryCache,
            mock_rtps_stateful_reader::MockRtpsStatefulReader,
        },
        utils::shared_object::DdsShared,
    };
    use dds_api::{
        dcps_psm::{ANY_INSTANCE_STATE, ANY_SAMPLE_STATE, ANY_VIEW_STATE},
        infrastructure::qos_policy::{DeadlineQosPolicy, HistoryQosPolicy},
    };
    use mockall::mock;
    use std::io::Write;

    pub struct ManualTimer {
        on_deadline: Option<Box<dyn FnMut() + Send + Sync>>,
    }

    impl ManualTimer {
        pub fn trigger(&mut self) {
            if let Some(f) = &mut self.on_deadline {
                f()
            }
            self.on_deadline = None;
        }
    }

    impl Timer for ManualTimer {
        fn new(_duration: Duration) -> Self {
            ManualTimer { on_deadline: None }
        }

        fn reset(&mut self) {
            self.on_deadline = None;
        }

        fn on_deadline<F>(&mut self, f: F)
        where
            F: FnMut() + Send + Sync + 'static,
        {
            self.on_deadline = Some(Box::new(f));
        }
    }

    struct UserData(u8);

    impl DdsType for UserData {
        fn type_name() -> &'static str {
            "UserData"
        }

        fn has_key() -> bool {
            false
        }
    }

    impl<'de> DdsDeserialize<'de> for UserData {
        fn deserialize(buf: &mut &'de [u8]) -> dds_api::return_type::DdsResult<Self> {
            Ok(UserData(buf[0]))
        }
    }

    impl DdsSerialize for UserData {
        fn serialize<W: Write, E: Endianness>(
            &self,
            mut writer: W,
        ) -> dds_api::return_type::DdsResult<()> {
            writer
                .write(&[self.0])
                .map(|_| ())
                .map_err(|e| DdsError::PreconditionNotMet(format!("{}", e)))
        }
    }

    fn cache_change(value: u8, sn: SequenceNumber) -> MockRtpsCacheChange {
        let mut cache_change = MockRtpsCacheChange::new();
        cache_change.expect_data_value().return_const(vec![value]);
        cache_change.expect_sequence_number().return_const(sn);

        cache_change
    }

    fn reader_with_changes<T: Timer>(
        changes: Vec<MockRtpsCacheChange>,
    ) -> DataReaderAttributes<MockRtps, T> {
        let mut history_cache = MockRtpsHistoryCache::new();
        history_cache.expect_changes().return_const(changes);

        let mut stateful_reader = MockRtpsStatefulReader::new();
        stateful_reader
            .expect_reader_cache()
            .return_var(history_cache);

        DataReaderAttributes::new(
            DataReaderQos {
                history: HistoryQosPolicy {
                    kind: HistoryQosPolicyKind::KeepAllHistoryQos,
                    depth: 0,
                },
                ..Default::default()
            },
            RtpsReader::Stateful(stateful_reader),
            DdsShared::new(TopicAttributes::new(
                Default::default(),
                "type_name",
                "topic_name",
                DdsWeak::new(),
            )),
            None,
            DdsWeak::new(),
        )
    }

    #[test]
    fn read_all_samples() {
        let reader = DdsShared::new(reader_with_changes(vec![
            cache_change(1, 1),
            cache_change(0, 2),
            cache_change(2, 3),
            cache_change(5, 4),
        ]));
        let reader_proxy =
            DataReaderProxy::<UserData, MockRtps, ManualTimer>::new(reader.downgrade());

        let all_samples = reader_proxy
            .read(
                i32::MAX,
                ANY_SAMPLE_STATE,
                ANY_VIEW_STATE,
                ANY_INSTANCE_STATE,
            )
            .unwrap();
        assert_eq!(4, all_samples.len());
        assert_eq!(
            vec![1, 0, 2, 5],
            all_samples.into_iter().map(|s| s.0 .0).collect::<Vec<_>>()
        );
    }

    #[test]
    fn read_only_unread() {
        let reader = DdsShared::new(reader_with_changes(vec![cache_change(1, 1)]));
        let reader_proxy =
            DataReaderProxy::<UserData, MockRtps, ManualTimer>::new(reader.downgrade());

        let unread_samples = reader_proxy
            .read(
                i32::MAX,
                NOT_READ_SAMPLE_STATE,
                ANY_VIEW_STATE,
                ANY_INSTANCE_STATE,
            )
            .unwrap();

        assert_eq!(1, unread_samples.len());

        assert!(reader_proxy
            .read(
                i32::MAX,
                NOT_READ_SAMPLE_STATE,
                ANY_VIEW_STATE,
                ANY_INSTANCE_STATE,
            )
            .is_err());
    }

    #[test]
    fn on_missed_deadline_increases_total_count() {
        let reader = {
            let mut reader = reader_with_changes(vec![]);
            reader.qos = DataReaderQos {
                deadline: DeadlineQosPolicy {
                    period: dds_api::dcps_psm::Duration::new(1, 0),
                },
                ..Default::default()
            };
            DdsShared::new(reader)
        };
        let reader_proxy =
            DataReaderProxy::<UserData, MockRtps, ManualTimer>::new(reader.downgrade());

        assert_eq!(
            0,
            reader_proxy
                .get_requested_deadline_missed_status()
                .unwrap()
                .total_count
        );

        DataReaderAttributes::on_data_received(reader.clone()).unwrap();

        assert_eq!(
            0,
            reader_proxy
                .get_requested_deadline_missed_status()
                .unwrap()
                .total_count
        );

        reader.deadline_timer.write_lock().trigger();

        assert_eq!(
            1,
            reader_proxy
                .get_requested_deadline_missed_status()
                .unwrap()
                .total_count
        );
    }

    mock! {
        Listener {}
        impl DataReaderListener for Listener {
            type Foo = UserData;

            fn on_data_available(&self, _the_reader: &dyn DataReader<UserData>);
            fn on_sample_rejected(
                &self,
                _the_reader: &dyn DataReader<UserData>,
                _status: SampleRejectedStatus,
            );
            fn on_liveliness_changed(
                &self,
                _the_reader: &dyn DataReader<UserData>,
                _status: LivelinessChangedStatus,
            );
            fn on_requested_deadline_missed(
                &self,
                _the_reader: &dyn DataReader<UserData>,
                _status: RequestedDeadlineMissedStatus,
            );
            fn on_requested_incompatible_qos(
                &self,
                _the_reader: &dyn DataReader<UserData>,
                _status: RequestedIncompatibleQosStatus,
            );
            fn on_subscription_matched(
                &self,
                _the_reader: &dyn DataReader<UserData>,
                _status: SubscriptionMatchedStatus,
            );
            fn on_sample_lost(&self, _the_reader: &dyn DataReader<UserData>, _status: SampleLostStatus);
        }
    }

    #[test]
    fn on_deadline_missed_calls_listener() {
        let reader = {
            let mut reader = reader_with_changes(vec![]);
            reader.qos = DataReaderQos {
                deadline: DeadlineQosPolicy {
                    period: dds_api::dcps_psm::Duration::new(1, 0),
                },
                ..Default::default()
            };
            DdsShared::new(reader)
        };
        let reader_proxy =
            DataReaderProxy::<UserData, MockRtps, ManualTimer>::new(reader.downgrade());

        DataReaderAttributes::on_data_received(reader.clone()).unwrap();

        let mut listener = MockListener::new();
        listener
            .expect_on_requested_deadline_missed()
            .once()
            .return_const(());
        reader_proxy
            .set_listener(Some(Box::new(listener)), 0)
            .unwrap();

        reader.deadline_timer.write_lock().trigger();
    }

    #[test]
    fn receiving_data_triggers_status_change() {
        let reader = {
            let mut reader = reader_with_changes(vec![]);
            reader.qos = DataReaderQos {
                deadline: DeadlineQosPolicy {
                    period: dds_api::dcps_psm::Duration::new(1, 0),
                },
                ..Default::default()
            };
            DdsShared::new(reader)
        };
        let reader_proxy =
            DataReaderProxy::<UserData, MockRtps, ManualTimer>::new(reader.downgrade());

        DataReaderAttributes::on_data_received(reader.clone()).unwrap();

        assert!(reader_proxy.get_status_changes().unwrap() & DATA_AVAILABLE_STATUS > 0);
    }

    #[test]
    fn on_data_available_listener_resets_status_change() {
        let reader = {
            let mut reader = reader_with_changes(vec![]);
            reader.qos = DataReaderQos {
                deadline: DeadlineQosPolicy {
                    period: dds_api::dcps_psm::Duration::new(1, 0),
                },
                ..Default::default()
            };
            DdsShared::new(reader)
        };
        let reader_proxy =
            DataReaderProxy::<UserData, MockRtps, ManualTimer>::new(reader.downgrade());

        let listener = {
            let mut listener = MockListener::new();
            let reader_proxy = reader_proxy.clone();
            listener
                .expect_on_data_available()
                .once()
                .withf(move |_| {
                    reader_proxy.get_status_changes().unwrap() & DATA_AVAILABLE_STATUS == 0
                })
                .return_const(());
            listener
        };
        reader_proxy
            .set_listener(Some(Box::new(listener)), 0)
            .unwrap();

        DataReaderAttributes::on_data_received(reader.clone()).unwrap();

        assert_eq!(
            0,
            reader_proxy.get_status_changes().unwrap() & DATA_AVAILABLE_STATUS
        );
    }

    #[test]
    fn deadline_missed_triggers_status_change() {
        let reader = {
            let mut reader = reader_with_changes(vec![]);
            reader.qos = DataReaderQos {
                deadline: DeadlineQosPolicy {
                    period: dds_api::dcps_psm::Duration::new(1, 0),
                },
                ..Default::default()
            };
            DdsShared::new(reader)
        };
        let reader_proxy =
            DataReaderProxy::<UserData, MockRtps, ManualTimer>::new(reader.downgrade());

        DataReaderAttributes::on_data_received(reader.clone()).unwrap();
        reader.deadline_timer.write_lock().trigger();

        assert!(reader_proxy.get_status_changes().unwrap() & REQUESTED_DEADLINE_MISSED_STATUS > 0);
    }

    #[test]
    fn on_deadline_missed_listener_resets_status_changed() {
        let reader = {
            let mut reader = reader_with_changes(vec![]);
            reader.qos = DataReaderQos {
                deadline: DeadlineQosPolicy {
                    period: dds_api::dcps_psm::Duration::new(1, 0),
                },
                ..Default::default()
            };
            DdsShared::new(reader)
        };
        let reader_proxy =
            DataReaderProxy::<UserData, MockRtps, ManualTimer>::new(reader.downgrade());

        let listener = {
            let mut listener = MockListener::new();
            let reader_proxy = reader_proxy.clone();
            listener
                .expect_on_requested_deadline_missed()
                .once()
                .withf(move |_, _| {
                    reader_proxy.get_status_changes().unwrap() & REQUESTED_DEADLINE_MISSED_STATUS
                        == 0
                })
                .return_const(());
            listener.expect_on_data_available().once().return_const(());
            listener
        };
        reader_proxy
            .set_listener(Some(Box::new(listener)), 0)
            .unwrap();

        DataReaderAttributes::on_data_received(reader.clone()).unwrap();
        reader.deadline_timer.write_lock().trigger();

        assert_eq!(
            0,
            reader_proxy.get_status_changes().unwrap() & REQUESTED_DEADLINE_MISSED_STATUS
        );
    }

    fn reader_with_max_depth<T: Timer>(
        max_depth: i32,
        changes: Vec<MockRtpsCacheChange>,
    ) -> DataReaderAttributes<MockRtps, T> {
        let mut history_cache = MockRtpsHistoryCache::new();
        history_cache.expect_changes().return_const(changes);

        let mut stateful_reader = MockRtpsStatefulReader::new();
        stateful_reader
            .expect_reader_cache()
            .return_var(history_cache);

        DataReaderAttributes::new(
            DataReaderQos {
                history: HistoryQosPolicy {
                    kind: HistoryQosPolicyKind::KeepLastHistoryQoS,
                    depth: max_depth,
                },
                ..Default::default()
            },
            RtpsReader::Stateful(stateful_reader),
            DdsShared::new(TopicAttributes::new(
                Default::default(),
                "type_name",
                "topic_name",
                DdsWeak::new(),
            )),
            None,
            DdsWeak::new(),
        )
    }

    #[test]
    fn keep_last_qos() {
        let reader = {
            let reader = reader_with_max_depth::<ManualTimer>(
                2,
                vec![
                    cache_change(1, 1),
                    cache_change(2, 2),
                    cache_change(3, 3),
                    cache_change(4, 4),
                ],
            );

            reader
                .rtps_reader
                .write_lock()
                .reader_cache()
                .expect_remove_change_()
                .returning(|f| {
                    assert!(f(&cache_change(1, 1)) == true);
                    assert!(f(&cache_change(2, 2)) == true);
                    assert!(f(&cache_change(3, 3)) == false);
                    assert!(f(&cache_change(4, 4)) == false);
                    ()
                });

            DdsShared::new(reader)
        };

        DataReaderAttributes::on_data_received(reader.clone()).unwrap();
    }
}
