use std::{
    net::UdpSocket,
    sync::{
        atomic::{self, AtomicBool},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
};

use crate::{
    domain::domain_participant_factory::DomainId,
    implementation::{
        rtps::participant::RtpsParticipant,
        rtps_udp_psm::udp_transport::{RtpsUdpPsm, UdpTransport},
        utils::shared_object::DdsShared,
    },
    infrastructure::{error::DdsResult, qos::DomainParticipantQos},
};

use super::{configuration::DustDdsConfiguration, domain_participant_impl::DomainParticipantImpl};

pub struct DcpsService {
    participant: DdsShared<DomainParticipantImpl>,
    quit: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl DcpsService {
    pub fn new(
        rtps_participant: RtpsParticipant,
        domain_id: DomainId,
        configuration: DustDdsConfiguration,
        domain_participant_qos: DomainParticipantQos,
        mut transport: RtpsUdpPsm,
    ) -> DdsResult<Self> {
        let announcer_condvar = Arc::new(Condvar::new());
        let participant = DomainParticipantImpl::new(
            rtps_participant,
            domain_id,
            configuration.domain_tag,
            domain_participant_qos,
            transport.metatraffic_unicast_locator_list(),
            transport.metatraffic_multicast_locator_list(),
            announcer_condvar.clone(),
        );

        participant.enable()?;

        participant.create_builtins()?;

        let quit = Arc::new(AtomicBool::new(false));
        let mut threads = Vec::new();
        // //////////// SPDP Communication

        // ////////////// SPDP participant discovery
        {
            let domain_participant = participant.clone();
            let mut metatraffic_multicast_transport =
                transport.metatraffic_multicast_transport().unwrap();
            let task_quit = quit.clone();

            threads.push(std::thread::spawn(move || loop {
                if task_quit.load(atomic::Ordering::SeqCst) {
                    break;
                }

                if let Some((locator, message)) = metatraffic_multicast_transport
                    .read(Some(std::time::Duration::from_millis(1000)))
                {
                    domain_participant.receive_built_in_data(locator, message);
                }
            }));
        }

        // ////////////// SPDP builtin endpoint configuration
        {
            let domain_participant = participant.clone();

            let task_quit = quit.clone();

            threads.push(std::thread::spawn(move || loop {
                if task_quit.load(atomic::Ordering::SeqCst) {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(500));

                match domain_participant.discover_matched_participants() {
                    Ok(()) => (),
                    Err(e) => println!("spdp discovery failed: {:?}", e),
                }
            }));
        }

        // //////////// Unicast metatraffic Communication receive
        {
            let domain_participant = participant.clone();
            let mut metatraffic_unicast_transport =
                transport.metatraffic_unicast_transport().unwrap();
            let task_quit = quit.clone();
            threads.push(std::thread::spawn(move || loop {
                if task_quit.load(atomic::Ordering::SeqCst) {
                    break;
                }

                if let Some((locator, message)) =
                    metatraffic_unicast_transport.read(Some(std::time::Duration::from_millis(1000)))
                {
                    domain_participant.receive_built_in_data(locator, message);
                }
            }));
        }

        // //////////// Unicast metatraffic Communication send
        {
            let domain_participant = participant.clone();
            let socket = UdpSocket::bind("0.0.0.0:0000").unwrap();

            let mut metatraffic_unicast_transport_send = UdpTransport::new(socket);
            let task_quit = quit.clone();
            threads.push(std::thread::spawn(move || loop {
                if task_quit.load(atomic::Ordering::SeqCst) {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(500));

                domain_participant.send_built_in_data(&mut metatraffic_unicast_transport_send);
            }));
        }

        // ////////////// SEDP user-defined endpoint configuration
        {
            let domain_participant = participant.clone();

            let task_quit = quit.clone();
            threads.push(std::thread::spawn(move || loop {
                if task_quit.load(atomic::Ordering::SeqCst) {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(500));

                match domain_participant.discover_matched_writers() {
                    Ok(()) => (),
                    Err(e) => println!("sedp writer discovery failed: {:?}", e),
                }
                match domain_participant.discover_matched_readers() {
                    Ok(()) => (),
                    Err(e) => println!("sedp reader discovery failed: {:?}", e),
                }
            }));
        }

        // //////////// User-defined Communication receive
        {
            let domain_participant = participant.clone();
            let mut default_unicast_transport = transport.default_unicast_transport().unwrap();
            let task_quit = quit.clone();
            threads.push(std::thread::spawn(move || loop {
                if task_quit.load(atomic::Ordering::SeqCst) {
                    break;
                }

                domain_participant.send_user_defined_data(&mut default_unicast_transport);
                if let Some((locator, message)) =
                    default_unicast_transport.read(Some(std::time::Duration::from_millis(1000)))
                {
                    domain_participant.receive_user_defined_data(locator, message);
                }
            }));
        }

        // //////////// User-defined Communication send
        {
            let domain_participant = participant.clone();
            let socket = UdpSocket::bind("0.0.0.0:0000").unwrap();
            let mut default_unicast_transport_send = UdpTransport::new(socket);
            let task_quit = quit.clone();
            threads.push(std::thread::spawn(move || loop {
                if task_quit.load(atomic::Ordering::SeqCst) {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(500));

                domain_participant.send_user_defined_data(&mut default_unicast_transport_send);
            }));
        }

        // //////////// Announce participant
        let domain_participant = participant.clone();
        let task_quit = quit.clone();
        let lock = Mutex::new(());

        threads.push(std::thread::spawn(move || {
            // TODO: Temporary solution to ensure tests pass by announcing as soon as possible
            domain_participant.announce_participant().ok();
            loop {
                if task_quit.load(atomic::Ordering::SeqCst) {
                    break;
                }

                let var_lock = lock.lock().unwrap();
                let _r =
                    announcer_condvar.wait_timeout(var_lock, std::time::Duration::from_secs(5));

                match domain_participant.announce_participant() {
                    Ok(_) => (),
                    Err(e) => println!("participant announcement failed: {:?}", e),
                }
            }
        }));

        Ok(DcpsService {
            participant,
            quit,
            threads,
        })
    }

    pub fn participant(&self) -> &DdsShared<DomainParticipantImpl> {
        &self.participant
    }

    pub fn shutdown_tasks(&mut self) {
        self.quit.store(true, atomic::Ordering::SeqCst);

        while let Some(thread) = self.threads.pop() {
            thread.join().unwrap();
        }
    }
}
