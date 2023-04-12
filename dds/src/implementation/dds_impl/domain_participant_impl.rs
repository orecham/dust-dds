use crate::{
    builtin_topics::BuiltInTopicKey,
    domain::{
        domain_participant_factory::DomainId,
        domain_participant_listener::DomainParticipantListener,
    },
    implementation::{
        data_representation_builtin_endpoints::{
            discovered_reader_data::{DiscoveredReaderData, DCPS_SUBSCRIPTION},
            discovered_topic_data::{DiscoveredTopicData, DCPS_TOPIC},
            discovered_writer_data::{DiscoveredWriterData, DCPS_PUBLICATION},
            spdp_discovered_participant_data::{
                ParticipantProxy, SpdpDiscoveredParticipantData, DCPS_PARTICIPANT,
            },
        },
        rtps::{
            discovery_types::{BuiltinEndpointQos, BuiltinEndpointSet},
            group::RtpsGroup,
            messages::RtpsMessage,
            participant::RtpsParticipant,
            types::{
                Count, EntityId, EntityKey, Guid, Locator, ProtocolVersion, VendorId,
                BUILT_IN_READER_WITH_KEY, BUILT_IN_TOPIC, BUILT_IN_WRITER_WITH_KEY,
                ENTITYID_PARTICIPANT, USER_DEFINED_READER_GROUP, USER_DEFINED_TOPIC,
                USER_DEFINED_WRITER_GROUP,
            },
        },
        utils::{
            condvar::DdsCondvar,
            iterator::DdsIterator,
            shared_object::{DdsRwLock, DdsShared},
            timer_factory::{Timer, TimerFactory},
        },
    },
    infrastructure::{
        instance::InstanceHandle,
        qos::QosKind,
        status::{StatusKind, NO_STATUS},
        time::DurationKind,
    },
    publication::publisher_listener::PublisherListener,
    subscription::{
        sample_info::{
            InstanceStateKind, SampleStateKind, ANY_INSTANCE_STATE, ANY_SAMPLE_STATE,
            ANY_VIEW_STATE,
        },
        subscriber_listener::SubscriberListener,
    },
    topic_definition::type_support::{DdsSerialize, DdsType, LittleEndian},
    {
        builtin_topics::{ParticipantBuiltinTopicData, TopicBuiltinTopicData},
        infrastructure::{
            error::{DdsError, DdsResult},
            qos::{DomainParticipantQos, PublisherQos, SubscriberQos, TopicQos},
            time::{Duration, Time},
        },
    },
};

use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc::SyncSender,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    any_topic_listener::AnyTopicListener, builtin_publisher::BuiltinPublisher,
    builtin_subscriber::BuiltInSubscriber, message_receiver::MessageReceiver,
    participant_discovery::ParticipantDiscovery, status_condition_impl::StatusConditionImpl,
    status_listener::StatusListener, topic_impl::TopicImpl,
    user_defined_publisher::UserDefinedPublisher, user_defined_subscriber::UserDefinedSubscriber,
};

pub const ENTITYID_SPDP_BUILTIN_PARTICIPANT_WRITER: EntityId =
    EntityId::new(EntityKey::new([0x00, 0x01, 0x00]), BUILT_IN_WRITER_WITH_KEY);

pub const ENTITYID_SPDP_BUILTIN_PARTICIPANT_READER: EntityId =
    EntityId::new(EntityKey::new([0x00, 0x01, 0x00]), BUILT_IN_READER_WITH_KEY);

pub const ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER: EntityId =
    EntityId::new(EntityKey::new([0, 0, 0x02]), BUILT_IN_WRITER_WITH_KEY);

pub const ENTITYID_SEDP_BUILTIN_TOPICS_DETECTOR: EntityId =
    EntityId::new(EntityKey::new([0, 0, 0x02]), BUILT_IN_READER_WITH_KEY);

pub const ENTITYID_SEDP_BUILTIN_PUBLICATIONS_ANNOUNCER: EntityId =
    EntityId::new(EntityKey::new([0, 0, 0x03]), BUILT_IN_WRITER_WITH_KEY);

pub const ENTITYID_SEDP_BUILTIN_PUBLICATIONS_DETECTOR: EntityId =
    EntityId::new(EntityKey::new([0, 0, 0x03]), BUILT_IN_READER_WITH_KEY);

pub const ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_ANNOUNCER: EntityId =
    EntityId::new(EntityKey::new([0, 0, 0x04]), BUILT_IN_WRITER_WITH_KEY);

pub const ENTITYID_SEDP_BUILTIN_SUBSCRIPTIONS_DETECTOR: EntityId =
    EntityId::new(EntityKey::new([0, 0, 0x04]), BUILT_IN_READER_WITH_KEY);

pub enum AnnounceKind {
    CreatedDataReader(DiscoveredReaderData),
    CreatedDataWriter(DiscoveredWriterData),
    CratedTopic(DiscoveredTopicData),
    DeletedDataReader(InstanceHandle),
    DeletedDataWriter(InstanceHandle),
    DeletedParticipant,
}

pub struct DomainParticipantImpl {
    rtps_participant: RtpsParticipant,
    domain_id: DomainId,
    domain_tag: String,
    qos: DdsRwLock<DomainParticipantQos>,
    builtin_subscriber: DdsShared<BuiltInSubscriber>,
    builtin_publisher: DdsShared<BuiltinPublisher>,
    user_defined_subscriber_list: DdsRwLock<Vec<DdsShared<UserDefinedSubscriber>>>,
    user_defined_subscriber_counter: AtomicU8,
    default_subscriber_qos: DdsRwLock<SubscriberQos>,
    user_defined_publisher_list: DdsRwLock<Vec<DdsShared<UserDefinedPublisher>>>,
    user_defined_publisher_counter: AtomicU8,
    default_publisher_qos: DdsRwLock<PublisherQos>,
    topic_list: DdsRwLock<Vec<DdsShared<TopicImpl>>>,
    user_defined_topic_counter: AtomicU8,
    default_topic_qos: DdsRwLock<TopicQos>,
    manual_liveliness_count: Count,
    lease_duration: Duration,
    discovered_participant_list: DdsRwLock<HashMap<InstanceHandle, SpdpDiscoveredParticipantData>>,
    discovered_topic_list: DdsRwLock<HashMap<InstanceHandle, TopicBuiltinTopicData>>,
    enabled: DdsRwLock<bool>,
    status_listener: DdsRwLock<StatusListener<dyn DomainParticipantListener + Send + Sync>>,
    user_defined_data_send_condvar: DdsCondvar,
    topic_find_condvar: DdsCondvar,
    sedp_condvar: DdsCondvar,
    ignored_participants: DdsRwLock<HashSet<InstanceHandle>>,
    ignored_publications: DdsRwLock<HashSet<InstanceHandle>>,
    ignored_subcriptions: DdsRwLock<HashSet<InstanceHandle>>,
    data_max_size_serialized: usize,
    _timer_factory: TimerFactory,
    timer: DdsShared<DdsRwLock<Timer>>,
    status_condition: DdsShared<DdsRwLock<StatusConditionImpl>>,
    announce_sender: SyncSender<AnnounceKind>,
}

impl DomainParticipantImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rtps_participant: RtpsParticipant,
        domain_id: DomainId,
        domain_tag: String,
        domain_participant_qos: DomainParticipantQos,
        listener: Option<Box<dyn DomainParticipantListener + Send + Sync>>,
        mask: &[StatusKind],
        spdp_discovery_locator_list: &[Locator],
        sedp_condvar: DdsCondvar,
        user_defined_data_send_condvar: DdsCondvar,
        data_max_size_serialized: usize,
        announce_sender: SyncSender<AnnounceKind>,
    ) -> DdsShared<Self> {
        let lease_duration = Duration::new(100, 0);
        let guid_prefix = rtps_participant.guid().prefix();

        let spdp_topic_entity_id = EntityId::new(EntityKey::new([0, 0, 0]), BUILT_IN_TOPIC);
        let spdp_topic_guid = Guid::new(guid_prefix, spdp_topic_entity_id);
        let spdp_topic_participant = TopicImpl::new(
            spdp_topic_guid,
            TopicQos::default(),
            SpdpDiscoveredParticipantData::type_name(),
            DCPS_PARTICIPANT,
            None,
            &[],
            announce_sender.clone(),
        );

        let sedp_topics_entity_id = EntityId::new(EntityKey::new([0, 0, 1]), BUILT_IN_TOPIC);
        let sedp_topics_guid = Guid::new(guid_prefix, sedp_topics_entity_id);
        let sedp_topic_topics = TopicImpl::new(
            sedp_topics_guid,
            TopicQos::default(),
            DiscoveredTopicData::type_name(),
            DCPS_TOPIC,
            None,
            &[],
            announce_sender.clone(),
        );

        let sedp_publications_entity_id = EntityId::new(EntityKey::new([0, 0, 2]), BUILT_IN_TOPIC);
        let sedp_publications_guid = Guid::new(guid_prefix, sedp_publications_entity_id);
        let sedp_topic_publications = TopicImpl::new(
            sedp_publications_guid,
            TopicQos::default(),
            DiscoveredWriterData::type_name(),
            DCPS_PUBLICATION,
            None,
            &[],
            announce_sender.clone(),
        );

        let sedp_subscriptions_entity_id = EntityId::new(EntityKey::new([0, 0, 2]), BUILT_IN_TOPIC);
        let sedp_subscriptions_guid = Guid::new(guid_prefix, sedp_subscriptions_entity_id);
        let sedp_topic_subscriptions = TopicImpl::new(
            sedp_subscriptions_guid,
            TopicQos::default(),
            DiscoveredReaderData::type_name(),
            DCPS_SUBSCRIPTION,
            None,
            &[],
            announce_sender.clone(),
        );

        let builtin_subscriber = BuiltInSubscriber::new(
            guid_prefix,
            spdp_topic_participant,
            sedp_topic_topics.clone(),
            sedp_topic_publications.clone(),
            sedp_topic_subscriptions.clone(),
        );

        let builtin_publisher = BuiltinPublisher::new(
            guid_prefix,
            sedp_topic_topics,
            sedp_topic_publications,
            sedp_topic_subscriptions,
            spdp_discovery_locator_list,
            sedp_condvar.clone(),
        );

        let timer_factory = TimerFactory::new();
        let timer = timer_factory.create_timer();

        DdsShared::new(DomainParticipantImpl {
            rtps_participant,
            domain_id,
            domain_tag,
            qos: DdsRwLock::new(domain_participant_qos),
            builtin_subscriber,
            builtin_publisher,
            user_defined_subscriber_list: DdsRwLock::new(Vec::new()),
            user_defined_subscriber_counter: AtomicU8::new(0),
            default_subscriber_qos: DdsRwLock::new(SubscriberQos::default()),
            user_defined_publisher_list: DdsRwLock::new(Vec::new()),
            user_defined_publisher_counter: AtomicU8::new(0),
            default_publisher_qos: DdsRwLock::new(PublisherQos::default()),
            topic_list: DdsRwLock::new(Vec::new()),
            user_defined_topic_counter: AtomicU8::new(0),
            default_topic_qos: DdsRwLock::new(TopicQos::default()),
            manual_liveliness_count: Count::new(0),
            lease_duration,
            discovered_participant_list: DdsRwLock::new(HashMap::new()),
            discovered_topic_list: DdsRwLock::new(HashMap::new()),
            enabled: DdsRwLock::new(false),
            user_defined_data_send_condvar,
            status_listener: DdsRwLock::new(StatusListener::new(listener, mask)),
            topic_find_condvar: DdsCondvar::new(),
            sedp_condvar,
            ignored_participants: DdsRwLock::new(HashSet::new()),
            ignored_publications: DdsRwLock::new(HashSet::new()),
            ignored_subcriptions: DdsRwLock::new(HashSet::new()),
            data_max_size_serialized,
            _timer_factory: timer_factory,
            timer,
            status_condition: DdsShared::new(DdsRwLock::new(StatusConditionImpl::default())),
            announce_sender,
        })
    }

    pub fn guid(&self) -> Guid {
        self.rtps_participant.guid()
    }

    pub fn vendor_id(&self) -> VendorId {
        self.rtps_participant.vendor_id()
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.rtps_participant.protocol_version()
    }

    pub fn default_unicast_locator_list(&self) -> &[Locator] {
        self.rtps_participant.default_unicast_locator_list()
    }

    pub fn default_multicast_locator_list(&self) -> &[Locator] {
        self.rtps_participant.default_multicast_locator_list()
    }

    pub fn metatraffic_unicast_locator_list(&self) -> &[Locator] {
        self.rtps_participant.metatraffic_unicast_locator_list()
    }

    pub fn metatraffic_multicast_locator_list(&self) -> &[Locator] {
        self.rtps_participant.metatraffic_multicast_locator_list()
    }

    pub fn get_builtin_subscriber(&self) -> DdsShared<BuiltInSubscriber> {
        self.builtin_subscriber.clone()
    }

    pub fn get_builtin_publisher(&self) -> DdsShared<BuiltinPublisher> {
        self.builtin_publisher.clone()
    }

    pub fn get_current_time(&self) -> Time {
        let now_system_time = SystemTime::now();
        let unix_time = now_system_time
            .duration_since(UNIX_EPOCH)
            .expect("Clock time is before Unix epoch start");
        Time::new(unix_time.as_secs() as i32, unix_time.subsec_nanos())
    }

    pub fn is_enabled(&self) -> bool {
        *self.enabled.read_lock()
    }
}

impl DdsShared<DomainParticipantImpl> {
    pub fn create_publisher(
        &self,
        qos: QosKind<PublisherQos>,
        a_listener: Option<Box<dyn PublisherListener + Send + Sync>>,
        mask: &[StatusKind],
    ) -> DdsResult<DdsShared<UserDefinedPublisher>> {
        let publisher_qos = match qos {
            QosKind::Default => self.default_publisher_qos.read_lock().clone(),
            QosKind::Specific(q) => q,
        };
        let publisher_counter = self
            .user_defined_publisher_counter
            .fetch_add(1, Ordering::Relaxed);
        let entity_id = EntityId::new(
            EntityKey::new([publisher_counter, 0, 0]),
            USER_DEFINED_WRITER_GROUP,
        );
        let guid = Guid::new(self.rtps_participant.guid().prefix(), entity_id);
        let rtps_group = RtpsGroup::new(guid);
        let publisher_impl_shared = UserDefinedPublisher::new(
            publisher_qos,
            rtps_group,
            a_listener,
            mask,
            self.user_defined_data_send_condvar.clone(),
            self.data_max_size_serialized,
            self.announce_sender.clone(),
        );
        if *self.enabled.read_lock()
            && self
                .qos
                .read_lock()
                .entity_factory
                .autoenable_created_entities
        {
            publisher_impl_shared.enable()?;
        }

        self.user_defined_publisher_list
            .write_lock()
            .push(publisher_impl_shared.clone());

        Ok(publisher_impl_shared)
    }

    pub fn delete_publisher(&self, a_publisher_handle: InstanceHandle) -> DdsResult<()> {
        if self
            .user_defined_publisher_list
            .read_lock()
            .iter()
            .find(|&x| x.get_instance_handle() == a_publisher_handle)
            .ok_or_else(|| {
                DdsError::PreconditionNotMet(
                    "Publisher can only be deleted from its parent participant".to_string(),
                )
            })?
            .data_writer_list()
            .count()
            > 0
        {
            return Err(DdsError::PreconditionNotMet(
                "Publisher still contains data writers".to_string(),
            ));
        }

        self.user_defined_publisher_list
            .write_lock()
            .retain(|x| x.get_instance_handle() != a_publisher_handle);

        Ok(())
    }

    pub fn publisher_list(&self) -> DdsIterator<UserDefinedPublisher> {
        DdsIterator::new(self.user_defined_publisher_list.read_lock())
    }

    pub fn create_subscriber(
        &self,
        qos: QosKind<SubscriberQos>,
        a_listener: Option<Box<dyn SubscriberListener + Send + Sync>>,
        mask: &[StatusKind],
    ) -> DdsResult<DdsShared<UserDefinedSubscriber>> {
        let subscriber_qos = match qos {
            QosKind::Default => self.default_subscriber_qos.read_lock().clone(),
            QosKind::Specific(q) => q,
        };
        let subcriber_counter = self
            .user_defined_subscriber_counter
            .fetch_add(1, Ordering::Relaxed);
        let entity_id = EntityId::new(
            EntityKey::new([subcriber_counter, 0, 0]),
            USER_DEFINED_READER_GROUP,
        );
        let guid = Guid::new(self.rtps_participant.guid().prefix(), entity_id);
        let rtps_group = RtpsGroup::new(guid);
        let subscriber_shared = UserDefinedSubscriber::new(
            subscriber_qos,
            rtps_group,
            a_listener,
            mask,
            self.user_defined_data_send_condvar.clone(),
            self.announce_sender.clone(),
        );
        if *self.enabled.read_lock()
            && self
                .qos
                .read_lock()
                .entity_factory
                .autoenable_created_entities
        {
            subscriber_shared.enable()?;
        }

        self.user_defined_subscriber_list
            .write_lock()
            .push(subscriber_shared.clone());

        Ok(subscriber_shared)
    }

    pub fn delete_subscriber(&self, a_subscriber_handle: InstanceHandle) -> DdsResult<()> {
        if self
            .user_defined_subscriber_list
            .read_lock()
            .iter()
            .find(|&x| x.get_instance_handle() == a_subscriber_handle)
            .ok_or_else(|| {
                DdsError::PreconditionNotMet(
                    "Subscriber can only be deleted from its parent participant".to_string(),
                )
            })?
            .data_reader_list()
            .count()
            > 0
        {
            return Err(DdsError::PreconditionNotMet(
                "Subscriber still contains data readers".to_string(),
            ));
        }

        self.user_defined_subscriber_list
            .write_lock()
            .retain(|x| x.get_instance_handle() != a_subscriber_handle);

        Ok(())
    }

    pub fn subscriber_list(&self) -> DdsIterator<UserDefinedSubscriber> {
        DdsIterator::new(self.user_defined_subscriber_list.read_lock())
    }

    pub fn create_topic(
        &self,
        topic_name: &str,
        type_name: &'static str,
        qos: QosKind<TopicQos>,
        a_listener: Option<Box<dyn AnyTopicListener + Send + Sync>>,
        mask: &[StatusKind],
    ) -> DdsResult<DdsShared<TopicImpl>> {
        let topic_counter = self
            .user_defined_topic_counter
            .fetch_add(1, Ordering::Relaxed);
        let topic_guid = Guid::new(
            self.rtps_participant.guid().prefix(),
            EntityId::new(EntityKey::new([topic_counter, 0, 0]), USER_DEFINED_TOPIC),
        );
        let qos = match qos {
            QosKind::Default => self.default_topic_qos.read_lock().clone(),
            QosKind::Specific(q) => q,
        };

        // /////// Create topic
        let topic_shared = TopicImpl::new(
            topic_guid,
            qos,
            type_name,
            topic_name,
            a_listener,
            mask,
            self.announce_sender.clone(),
        );
        if *self.enabled.read_lock()
            && self
                .qos
                .read_lock()
                .entity_factory
                .autoenable_created_entities
        {
            topic_shared.enable()?;
        }

        self.topic_list.write_lock().push(topic_shared.clone());
        self.topic_find_condvar.notify_all();

        Ok(topic_shared)
    }

    pub fn delete_topic(&self, a_topic_handle: InstanceHandle) -> DdsResult<()> {
        let topic = self
            .topic_list
            .read_lock()
            .iter()
            .find(|&topic| topic.get_instance_handle() == a_topic_handle)
            .ok_or_else(|| {
                DdsError::PreconditionNotMet(
                    "Topic can only be deleted from its parent publisher".to_string(),
                )
            })?
            .clone();

        for publisher in self.user_defined_publisher_list.read_lock().iter() {
            if publisher.data_writer_list().any(|w| {
                w.get_type_name() == topic.get_type_name() && w.get_topic_name() == topic.get_name()
            }) {
                return Err(DdsError::PreconditionNotMet(
                    "Topic still attached to some data writer".to_string(),
                ));
            }
        }

        for subscriber in self.user_defined_subscriber_list.read_lock().iter() {
            if subscriber.data_reader_list().any(|r| {
                r.get_type_name() == topic.get_type_name() && r.get_topic_name() == topic.get_name()
            }) {
                return Err(DdsError::PreconditionNotMet(
                    "Topic still attached to some data reader".to_string(),
                ));
            }
        }

        self.topic_list
            .write_lock()
            .retain(|x| x.get_instance_handle() != a_topic_handle);
        Ok(())
    }

    pub fn topic_list(&self) -> DdsIterator<TopicImpl> {
        DdsIterator::new(self.topic_list.read_lock())
    }

    pub fn find_topic(
        &self,
        topic_name: &str,
        type_name: &'static str,
        timeout: Duration,
    ) -> DdsResult<DdsShared<TopicImpl>> {
        let start_time = self.get_current_time();

        while self.get_current_time() - start_time < timeout {
            // Check if a topic exists locally. If topic doesn't exist locally check if it has already been
            // discovered and, if so, create a new local topic representing the discovered topic
            if let Some(topic) =
                self.topic_list.read_lock().iter().find(|topic| {
                    topic.get_name() == topic_name && topic.get_type_name() == type_name
                })
            {
                return Ok(topic.clone());
            }

            // NOTE: Do not make this an else with the previous if because the topic_list read_lock is
            // kept and this enters a deadlock
            if let Some((_, discovered_topic_info)) = self
                .discovered_topic_list
                .read_lock()
                .iter()
                .find(|&(_, t)| t.name == topic_name && t.type_name == type_name)
            {
                let qos = TopicQos {
                    topic_data: discovered_topic_info.topic_data.clone(),
                    durability: discovered_topic_info.durability.clone(),
                    deadline: discovered_topic_info.deadline.clone(),
                    latency_budget: discovered_topic_info.latency_budget.clone(),
                    liveliness: discovered_topic_info.liveliness.clone(),
                    reliability: discovered_topic_info.reliability.clone(),
                    destination_order: discovered_topic_info.destination_order.clone(),
                    history: discovered_topic_info.history.clone(),
                    resource_limits: discovered_topic_info.resource_limits.clone(),
                    transport_priority: discovered_topic_info.transport_priority.clone(),
                    lifespan: discovered_topic_info.lifespan.clone(),
                    ownership: discovered_topic_info.ownership.clone(),
                };
                return self.create_topic(
                    &discovered_topic_info.name,
                    type_name,
                    QosKind::Specific(qos),
                    None,
                    NO_STATUS,
                );
            }
            // Block until timeout unless new topic is found or created
            let duration_until_timeout = (self.get_current_time() - start_time) - timeout;
            self.topic_find_condvar
                .wait_timeout(duration_until_timeout)
                .ok();
        }
        Err(DdsError::Timeout)
    }

    pub fn ignore_participant(&self, handle: InstanceHandle) {
        self.ignored_participants.write_lock().insert(handle);
        self.remove_discovered_participant(handle);
    }

    pub fn ignore_topic(&self, _handle: InstanceHandle) {
        todo!()
    }

    pub fn ignore_publication(&self, handle: InstanceHandle) {
        self.ignored_publications.write_lock().insert(handle);

        for subscriber in self.user_defined_subscriber_list.read_lock().iter() {
            subscriber.remove_matched_writer(handle, &mut self.status_listener.write_lock());
        }
    }

    pub fn ignore_subscription(&self, handle: InstanceHandle) {
        self.ignored_subcriptions.write_lock().insert(handle);
        for publisher in self.user_defined_publisher_list.read_lock().iter() {
            publisher.remove_matched_reader(handle, &mut self.status_listener.write_lock());
        }
    }

    pub fn get_domain_id(&self) -> DomainId {
        self.domain_id
    }

    pub fn delete_contained_entities(&self) -> DdsResult<()> {
        for user_defined_publisher in self.user_defined_publisher_list.write_lock().drain(..) {
            user_defined_publisher.delete_contained_entities()?;
        }

        for user_defined_subscriber in self.user_defined_subscriber_list.write_lock().drain(..) {
            user_defined_subscriber.delete_contained_entities()?;
        }

        self.topic_list.write_lock().clear();

        Ok(())
    }

    pub fn assert_liveliness(&self) -> DdsResult<()> {
        todo!()
    }

    pub fn set_default_publisher_qos(&self, qos: QosKind<PublisherQos>) -> DdsResult<()> {
        match qos {
            QosKind::Default => *self.default_publisher_qos.write_lock() = PublisherQos::default(),
            QosKind::Specific(q) => *self.default_publisher_qos.write_lock() = q,
        }

        Ok(())
    }

    pub fn get_default_publisher_qos(&self) -> PublisherQos {
        self.default_publisher_qos.read_lock().clone()
    }

    pub fn set_default_subscriber_qos(&self, qos: QosKind<SubscriberQos>) -> DdsResult<()> {
        match qos {
            QosKind::Default => {
                *self.default_subscriber_qos.write_lock() = SubscriberQos::default()
            }
            QosKind::Specific(q) => *self.default_subscriber_qos.write_lock() = q,
        }

        Ok(())
    }

    pub fn get_default_subscriber_qos(&self) -> SubscriberQos {
        self.default_subscriber_qos.read_lock().clone()
    }

    pub fn set_default_topic_qos(&self, qos: QosKind<TopicQos>) -> DdsResult<()> {
        match qos {
            QosKind::Default => *self.default_topic_qos.write_lock() = TopicQos::default(),
            QosKind::Specific(q) => {
                q.is_consistent()?;
                *self.default_topic_qos.write_lock() = q;
            }
        }

        Ok(())
    }

    pub fn get_default_topic_qos(&self) -> TopicQos {
        self.default_topic_qos.read_lock().clone()
    }

    pub fn get_discovered_participants(&self) -> DdsResult<Vec<InstanceHandle>> {
        Ok(self
            .discovered_participant_list
            .read_lock()
            .iter()
            .map(|(&key, _)| key)
            .collect())
    }

    pub fn get_discovered_participant_data(
        &self,
        participant_handle: InstanceHandle,
    ) -> DdsResult<ParticipantBuiltinTopicData> {
        Ok(self
            .discovered_participant_list
            .read_lock()
            .get(&participant_handle)
            .ok_or(DdsError::BadParameter)?
            .dds_participant_data
            .clone())
    }

    pub fn get_discovered_topics(&self) -> DdsResult<Vec<InstanceHandle>> {
        Ok(self
            .discovered_topic_list
            .read_lock()
            .keys()
            .cloned()
            .collect())
    }

    pub fn get_discovered_topic_data(
        &self,
        topic_handle: InstanceHandle,
    ) -> DdsResult<TopicBuiltinTopicData> {
        self.discovered_topic_list
            .read_lock()
            .get(&topic_handle)
            .cloned()
            .ok_or(DdsError::BadParameter)
    }

    pub fn contains_entity(&self, _a_handle: InstanceHandle) -> DdsResult<bool> {
        todo!()
    }

    pub fn set_qos(&self, qos: QosKind<DomainParticipantQos>) -> DdsResult<()> {
        *self.qos.write_lock() = match qos {
            QosKind::Default => DomainParticipantQos::default(),
            QosKind::Specific(q) => q,
        };
        self.announce_participant().ok();

        Ok(())
    }

    pub fn get_qos(&self) -> DomainParticipantQos {
        self.qos.read_lock().clone()
    }

    pub fn set_listener(
        &self,
        a_listener: Option<Box<dyn DomainParticipantListener + Send + Sync>>,
        mask: &[StatusKind],
    ) {
        *self.status_listener.write_lock() = StatusListener::new(a_listener, mask)
    }

    pub fn get_statuscondition(&self) -> DdsShared<DdsRwLock<StatusConditionImpl>> {
        self.status_condition.clone()
    }

    pub fn get_status_changes(&self) -> Vec<StatusKind> {
        self.status_condition.read_lock().get_status_changes()
    }

    pub fn enable(&self) -> DdsResult<()> {
        if !*self.enabled.read_lock() {
            *self.enabled.write_lock() = true;

            self.builtin_subscriber.enable()?;
            self.builtin_publisher.enable()?;

            if self
                .qos
                .read_lock()
                .entity_factory
                .autoenable_created_entities
            {
                for publisher in self.user_defined_publisher_list.read_lock().iter() {
                    publisher.enable()?;
                }

                for subscriber in self.user_defined_subscriber_list.read_lock().iter() {
                    subscriber.enable()?;
                }

                for topic in self.topic_list.read_lock().iter() {
                    topic.enable()?;
                }
            }

            self.announce_participant().ok();

            let this = self.clone();
            self.timer.write_lock().start_timer(
                DurationKind::Finite(Duration::new(5, 0)),
                InstanceHandle::new([0; 16]),
                move || {
                    this.announce_participant().ok();
                },
            );
        }
        Ok(())
    }

    fn announce_participant(&self) -> DdsResult<()> {
        let spdp_discovered_participant_data = SpdpDiscoveredParticipantData {
            dds_participant_data: ParticipantBuiltinTopicData {
                key: BuiltInTopicKey {
                    value: self.rtps_participant.guid().into(),
                },
                user_data: self.qos.read_lock().user_data.clone(),
            },
            participant_proxy: ParticipantProxy {
                domain_id: self.domain_id,
                domain_tag: self.domain_tag.clone(),
                protocol_version: self.rtps_participant.protocol_version(),
                guid_prefix: self.rtps_participant.guid().prefix(),
                vendor_id: self.rtps_participant.vendor_id(),
                expects_inline_qos: false,
                metatraffic_unicast_locator_list: self
                    .rtps_participant
                    .metatraffic_unicast_locator_list()
                    .to_vec(),
                metatraffic_multicast_locator_list: self
                    .rtps_participant
                    .metatraffic_multicast_locator_list()
                    .to_vec(),
                default_unicast_locator_list: self
                    .rtps_participant
                    .default_unicast_locator_list()
                    .to_vec(),
                default_multicast_locator_list: self
                    .rtps_participant
                    .default_multicast_locator_list()
                    .to_vec(),
                available_builtin_endpoints: BuiltinEndpointSet::default(),
                manual_liveliness_count: self.manual_liveliness_count,
                builtin_endpoint_qos: BuiltinEndpointQos::default(),
            },
            lease_duration: self.lease_duration.into(),
        };
        let mut serialized_data = Vec::new();
        spdp_discovered_participant_data.serialize::<_, LittleEndian>(&mut serialized_data)?;

        self.builtin_publisher
            .spdp_builtin_participant_writer()
            .write_w_timestamp(
                serialized_data,
                spdp_discovered_participant_data.get_serialized_key(),
                None,
                self.get_current_time(),
            )
    }

    pub fn add_discovered_participant(
        &self,
        discovered_participant_data: SpdpDiscoveredParticipantData,
    ) {
        if let Ok(participant_discovery) = ParticipantDiscovery::new(
            &discovered_participant_data,
            self.domain_id,
            &self.domain_tag,
        ) {
            if !self
                .ignored_participants
                .read_lock()
                .contains(&discovered_participant_data.get_serialized_key().into())
            {
                self.builtin_publisher
                    .sedp_builtin_publications_writer()
                    .add_matched_participant(&participant_discovery);

                let sedp_builtin_publication_reader_shared =
                    self.builtin_subscriber.sedp_builtin_publications_reader();
                sedp_builtin_publication_reader_shared
                    .add_matched_participant(&participant_discovery);

                self.builtin_publisher
                    .sedp_builtin_subscriptions_writer()
                    .add_matched_participant(&participant_discovery);

                let sedp_builtin_subscription_reader_shared =
                    self.builtin_subscriber.sedp_builtin_subscriptions_reader();
                sedp_builtin_subscription_reader_shared
                    .add_matched_participant(&participant_discovery);

                self.builtin_publisher
                    .sedp_builtin_topics_writer()
                    .add_matched_participant(&participant_discovery);

                let sedp_builtin_topic_reader_shared =
                    self.builtin_subscriber.sedp_builtin_topics_reader();
                sedp_builtin_topic_reader_shared.add_matched_participant(&participant_discovery);

                let this = self.clone();

                let lease_duration = Duration::from(discovered_participant_data.lease_duration);
                let handle = discovered_participant_data.get_serialized_key().into();
                self.timer.write_lock().start_timer(
                    DurationKind::Finite(lease_duration),
                    discovered_participant_data.get_serialized_key().into(),
                    move || this.remove_discovered_participant(handle),
                );

                self.discovered_participant_list.write_lock().insert(
                    discovered_participant_data.get_serialized_key().into(),
                    discovered_participant_data,
                );
            }
        }
    }

    pub fn remove_discovered_participant(&self, handle: InstanceHandle) {
        if let Some(discovered_participant_data) = self
            .discovered_participant_list
            .write_lock()
            .remove(&handle)
        {
            let participant_guid_prefix = discovered_participant_data.guid_prefix();
            self.builtin_subscriber
                .sedp_builtin_publications_reader()
                .remove_matched_participant(participant_guid_prefix);
            self.builtin_subscriber
                .sedp_builtin_subscriptions_reader()
                .remove_matched_participant(participant_guid_prefix);
            self.builtin_subscriber
                .sedp_builtin_topics_reader()
                .remove_matched_participant(participant_guid_prefix);
            self.builtin_publisher
                .sedp_builtin_publications_writer()
                .remove_matched_participant(participant_guid_prefix);
            self.builtin_publisher
                .sedp_builtin_subscriptions_writer()
                .remove_matched_participant(participant_guid_prefix);
            self.builtin_publisher
                .sedp_builtin_topics_writer()
                .remove_matched_participant(participant_guid_prefix);
        }
    }

    pub fn receive_built_in_data(
        &self,
        source_locator: Locator,
        message: RtpsMessage,
    ) -> DdsResult<()> {
        MessageReceiver::new(self.get_current_time()).process_message(
            self.rtps_participant.guid().prefix(),
            core::slice::from_ref(&self.builtin_publisher),
            core::slice::from_ref(&self.builtin_subscriber),
            source_locator,
            &message,
            &mut self.status_listener.write_lock(),
        )?;

        self.discover_matched_participants().ok();
        self.discover_matched_readers().ok();
        self.discover_matched_writers().ok();
        self.discover_matched_topics().ok();

        Ok(())
    }

    pub fn receive_user_defined_data(
        &self,
        source_locator: Locator,
        message: RtpsMessage,
    ) -> DdsResult<()> {
        MessageReceiver::new(self.get_current_time()).process_message(
            self.rtps_participant.guid().prefix(),
            self.user_defined_publisher_list.read_lock().as_slice(),
            self.user_defined_subscriber_list.read_lock().as_slice(),
            source_locator,
            &message,
            &mut self.status_listener.write_lock(),
        )
    }

    fn discover_matched_participants(&self) -> DdsResult<()> {
        let spdp_builtin_participant_data_reader =
            self.builtin_subscriber.spdp_builtin_participant_reader();

        while let Ok(samples) = spdp_builtin_participant_data_reader.read(
            1,
            &[SampleStateKind::NotRead],
            ANY_VIEW_STATE,
            ANY_INSTANCE_STATE,
            None,
        ) {
            for discovered_participant_data_sample in samples.into_iter() {
                if let Some(discovered_participant_data) = discovered_participant_data_sample.data {
                    self.add_discovered_participant(discovered_participant_data)
                }
            }
        }

        Ok(())
    }

    fn discover_matched_writers(&self) -> DdsResult<()> {
        let samples = self
            .builtin_subscriber
            .sedp_builtin_publications_reader()
            .read::<DiscoveredWriterData>(
            i32::MAX,
            ANY_SAMPLE_STATE,
            ANY_VIEW_STATE,
            ANY_INSTANCE_STATE,
            None,
        )?;

        for discovered_writer_data_sample in samples.into_iter() {
            match discovered_writer_data_sample.sample_info.instance_state {
                InstanceStateKind::Alive => {
                    if let Some(discovered_writer_data) = discovered_writer_data_sample.data {
                        if !self.ignored_publications.read_lock().contains(
                            &discovered_writer_data
                                .writer_proxy
                                .remote_writer_guid
                                .into(),
                        ) {
                            let remote_writer_guid_prefix = discovered_writer_data
                                .writer_proxy
                                .remote_writer_guid
                                .prefix();
                            let writer_parent_participant_guid =
                                Guid::new(remote_writer_guid_prefix, ENTITYID_PARTICIPANT);

                            if let Some(discovered_participant_data) = self
                                .discovered_participant_list
                                .read_lock()
                                .get(&writer_parent_participant_guid.into())
                            {
                                for subscriber in
                                    self.user_defined_subscriber_list.read_lock().iter()
                                {
                                    subscriber.add_matched_writer(
                                        &discovered_writer_data,
                                        discovered_participant_data.default_unicast_locator_list(),
                                        discovered_participant_data
                                            .default_multicast_locator_list(),
                                        &mut self.status_listener.write_lock(),
                                    );
                                }
                            }
                        }
                    }
                }
                InstanceStateKind::NotAliveDisposed => {
                    for subscriber in self.user_defined_subscriber_list.read_lock().iter() {
                        subscriber.remove_matched_writer(
                            discovered_writer_data_sample.sample_info.instance_handle,
                            &mut self.status_listener.write_lock(),
                        );
                    }
                }
                InstanceStateKind::NotAliveNoWriters => todo!(),
            }
        }

        Ok(())
    }

    fn discover_matched_readers(&self) -> DdsResult<()> {
        let samples = self
            .builtin_subscriber
            .sedp_builtin_subscriptions_reader()
            .read::<DiscoveredReaderData>(
                i32::MAX,
                ANY_SAMPLE_STATE,
                ANY_VIEW_STATE,
                ANY_INSTANCE_STATE,
                None,
            )?;

        for discovered_reader_data_sample in samples.into_iter() {
            match discovered_reader_data_sample.sample_info.instance_state {
                InstanceStateKind::Alive => {
                    if let Some(discovered_reader_data) = discovered_reader_data_sample.data {
                        if !self.ignored_subcriptions.read_lock().contains(
                            &discovered_reader_data
                                .reader_proxy
                                .remote_reader_guid
                                .into(),
                        ) {
                            let remote_reader_guid_prefix = discovered_reader_data
                                .reader_proxy
                                .remote_reader_guid
                                .prefix();
                            let reader_parent_participant_guid =
                                Guid::new(remote_reader_guid_prefix, ENTITYID_PARTICIPANT);

                            if let Some(discovered_participant_data) = self
                                .discovered_participant_list
                                .read_lock()
                                .get(&reader_parent_participant_guid.into())
                            {
                                for publisher in self.user_defined_publisher_list.read_lock().iter()
                                {
                                    publisher.add_matched_reader(
                                        &discovered_reader_data,
                                        discovered_participant_data.default_unicast_locator_list(),
                                        discovered_participant_data
                                            .default_multicast_locator_list(),
                                        &mut self.status_listener.write_lock(),
                                    );
                                }
                            }
                        }
                    }
                }
                InstanceStateKind::NotAliveDisposed => {
                    for publisher in self.user_defined_publisher_list.read_lock().iter() {
                        publisher.remove_matched_reader(
                            discovered_reader_data_sample.sample_info.instance_handle,
                            &mut self.status_listener.write_lock(),
                        )
                    }
                }

                InstanceStateKind::NotAliveNoWriters => todo!(),
            }
        }

        Ok(())
    }

    fn discover_matched_topics(&self) -> DdsResult<()> {
        while let Ok(samples) = self
            .builtin_subscriber
            .sedp_builtin_topics_reader()
            .read::<DiscoveredTopicData>(
            1,
            &[SampleStateKind::NotRead],
            ANY_VIEW_STATE,
            ANY_INSTANCE_STATE,
            None,
        ) {
            for sample in samples {
                if let Some(topic_data) = sample.data.as_ref() {
                    for topic in self.topic_list.read_lock().iter() {
                        topic.process_discovered_topic(
                            topic_data,
                            &mut self.status_listener.write_lock(),
                        );
                    }

                    self.discovered_topic_list.write_lock().insert(
                        topic_data.get_serialized_key().into(),
                        topic_data.topic_builtin_topic_data.clone(),
                    );

                    self.topic_find_condvar.notify_all();
                }
            }
        }

        Ok(())
    }

    pub fn update_communication_status(&self) -> DdsResult<()> {
        let now = self.get_current_time();
        for subscriber in self.user_defined_subscriber_list.read_lock().iter() {
            subscriber.update_communication_status(now, &mut self.status_listener.write_lock());
        }

        Ok(())
    }

    pub fn sedp_condvar(&self) -> &DdsCondvar {
        &self.sedp_condvar
    }

    pub fn user_defined_data_send_condvar(&self) -> &DdsCondvar {
        &self.user_defined_data_send_condvar
    }

    pub fn cancel_timers(&self) {
        self.timer.write_lock().cancel_timers()
    }
}
