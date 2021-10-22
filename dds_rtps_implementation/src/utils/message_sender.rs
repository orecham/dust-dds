use rust_rtps_pim::{
    messages::overall_structure::RtpsMessageHeader,
    structure::types::{GuidPrefix, Locator, ProtocolVersion, VendorId},
};
use rust_rtps_psm::messages::overall_structure::{RtpsMessageWrite, RtpsSubmessageTypeWrite};

use super::{
    shared_object::{rtps_shared_write_lock, RtpsShared},
    transport::TransportWrite,
};

pub trait RtpsSubmessageSender {
    fn create_submessages(&mut self) -> Vec<(Locator, Vec<RtpsSubmessageTypeWrite>)>;
}

// impl<T, U> RtpsSubmessageSender for T
// where
//     U: RtpsReaderLocator,
//     T: for<'a> StatelessWriterBehavior<'a, Vec<SequenceNumber>, ReaderLocator = U>,
// {
//     fn create_submessages(&mut self) -> Vec<(Locator, Vec<RtpsSubmessageWrite<'_>>)> {
//         let destined_submessages: Vec<(Locator, Vec<RtpsSubmessageWrite>)> = Vec::new();
//         let destined_submessages = RefCell::new(destined_submessages);
//         self.send_unsent_data(
//             |reader_locator, data| {
//                 let mut destined_submessages_borrow = destined_submessages.borrow_mut();
//                 match destined_submessages_borrow
//                     .iter_mut()
//                     .find(|(locator, _)| locator == reader_locator.locator())
//                 {
//                     Some((_, submessages)) => submessages.push(RtpsSubmessageType::Data(data)),
//                     None => destined_submessages_borrow.push((
//                         *reader_locator.locator(),
//                         vec![RtpsSubmessageType::Data(data)],
//                     )),
//                 }
//             },
//             |reader_locator, gap| {
//                 let mut destined_submessages_borrow = destined_submessages.borrow_mut();
//                 match destined_submessages_borrow
//                     .iter_mut()
//                     .find(|(locator, _)| locator == reader_locator.locator())
//                 {
//                     Some((_, submessages)) => submessages.push(RtpsSubmessageType::Gap(gap)),
//                     None => destined_submessages_borrow.push((
//                         *reader_locator.locator(),
//                         vec![RtpsSubmessageType::Gap(gap)],
//                     )),
//                 }
//             },
//         );
//         destined_submessages.take()
//     }
// }

pub fn send_data(
    protocol_version: &ProtocolVersion,
    vendor_id: &VendorId,
    guid_prefix: &GuidPrefix,
    writer_list: &[RtpsShared<impl RtpsSubmessageSender>],
    transport: &mut (impl TransportWrite + ?Sized),
) {
    for writer in writer_list {
        let mut writer_lock = rtps_shared_write_lock(&writer);
        let destined_submessages = writer_lock.create_submessages();
        for (dst_locator, submessages) in destined_submessages {
            let header = RtpsMessageHeader {
                protocol: rust_rtps_pim::messages::types::ProtocolId::PROTOCOL_RTPS,
                version: *protocol_version,
                vendor_id: *vendor_id,
                guid_prefix: *guid_prefix,
            };
            let message = RtpsMessageWrite::new(header, submessages);
            transport.write(&message, &dst_locator);
        }
    }
}

#[cfg(test)]
mod tests {
    // #[test]
    // fn submessage_send_empty() {
    //     struct MockBehavior;

    //     impl<'a> StatelessWriterBehavior<'a, Vec<SequenceNumber>> for MockBehavior {
    //         type ReaderLocator = MockReaderLocator;

    //         fn send_unsent_data(
    //             &'a mut self,
    //             _send_data: impl FnMut(&Self::ReaderLocator, DataSubmessage<'a, &'a [Parameter<'a>]>),
    //             _send_gap: impl FnMut(&Self::ReaderLocator, GapSubmessage<Vec<SequenceNumber>>),
    //         ) {
    //         }
    //     }

    //     let mut writer = MockBehavior;
    //     let destined_submessages: Vec<(Locator, Vec<RtpsSubmessageWrite>)> =
    //         writer.create_submessages();

    //     assert!(destined_submessages.is_empty());
    // }

    // #[test]
    // fn submessage_send_single_locator_send_only_data() {
    //     struct MockBehavior;

    //     const DATA_SUBMESSAGE1: DataSubmessage<&[Parameter]> = DataSubmessage {
    //         endianness_flag: false,
    //         inline_qos_flag: true,
    //         data_flag: true,
    //         key_flag: false,
    //         non_standard_payload_flag: false,
    //         reader_id: EntityIdSubmessageElement {
    //             value: ENTITYID_UNKNOWN,
    //         },
    //         writer_id: EntityIdSubmessageElement {
    //             value: ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER,
    //         },
    //         writer_sn: SequenceNumberSubmessageElement { value: 1 },
    //         inline_qos: ParameterListSubmessageElement::<&[Parameter]> { parameter: &[] },
    //         serialized_payload: SerializedDataSubmessageElement { value: &[1, 2, 3] },
    //     };

    //     const DATA_SUBMESSAGE2: DataSubmessage<&[Parameter]> = DataSubmessage {
    //         endianness_flag: false,
    //         inline_qos_flag: true,
    //         data_flag: true,
    //         key_flag: false,
    //         non_standard_payload_flag: false,
    //         reader_id: EntityIdSubmessageElement {
    //             value: ENTITYID_UNKNOWN,
    //         },
    //         writer_id: EntityIdSubmessageElement {
    //             value: ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER,
    //         },
    //         writer_sn: SequenceNumberSubmessageElement { value: 1 },
    //         inline_qos: ParameterListSubmessageElement::<&[Parameter]> { parameter: &[] },
    //         serialized_payload: SerializedDataSubmessageElement { value: &[4, 5, 6] },
    //     };

    //     impl<'a> StatelessWriterBehavior<'a, Vec<SequenceNumber>> for MockBehavior {
    //         type ReaderLocator = MockReaderLocator;

    //         fn send_unsent_data(
    //             &'a mut self,
    //             mut send_data: impl FnMut(&Self::ReaderLocator, DataSubmessage<'a, &'a [Parameter<'a>]>),
    //             _send_gap: impl FnMut(&Self::ReaderLocator, GapSubmessage<Vec<SequenceNumber>>),
    //         ) {
    //             send_data(&MockReaderLocator(LOCATOR_INVALID), DATA_SUBMESSAGE1);
    //             send_data(&MockReaderLocator(LOCATOR_INVALID), DATA_SUBMESSAGE2);
    //         }
    //     }

    //     let mut writer = MockBehavior;
    //     let destined_submessages: Vec<(Locator, Vec<RtpsSubmessageWrite>)> =
    //         writer.create_submessages();
    //     let (dst_locator, submessages) = &destined_submessages[0];

    //     assert_eq!(dst_locator, &LOCATOR_INVALID);
    //     assert_eq!(
    //         submessages,
    //         &vec![
    //             RtpsSubmessageType::Data(DATA_SUBMESSAGE1),
    //             RtpsSubmessageType::Data(DATA_SUBMESSAGE2)
    //         ]
    //     );
    // }

    // #[test]
    // fn submessage_send_multiple_locator_send_data_and_gap() {
    //     struct MockBehavior;

    //     const DATA_SUBMESSAGE1: DataSubmessage<&[Parameter]> = DataSubmessage {
    //         endianness_flag: false,
    //         inline_qos_flag: true,
    //         data_flag: true,
    //         key_flag: false,
    //         non_standard_payload_flag: false,
    //         reader_id: EntityIdSubmessageElement {
    //             value: ENTITYID_UNKNOWN,
    //         },
    //         writer_id: EntityIdSubmessageElement {
    //             value: ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER,
    //         },
    //         writer_sn: SequenceNumberSubmessageElement { value: 1 },
    //         inline_qos: ParameterListSubmessageElement::<&[Parameter]> { parameter: &[] },
    //         serialized_payload: SerializedDataSubmessageElement { value: &[1, 2, 3] },
    //     };

    //     const DATA_SUBMESSAGE2: DataSubmessage<&[Parameter]> = DataSubmessage {
    //         endianness_flag: false,
    //         inline_qos_flag: true,
    //         data_flag: true,
    //         key_flag: false,
    //         non_standard_payload_flag: false,
    //         reader_id: EntityIdSubmessageElement {
    //             value: ENTITYID_UNKNOWN,
    //         },
    //         writer_id: EntityIdSubmessageElement {
    //             value: ENTITYID_SEDP_BUILTIN_TOPICS_ANNOUNCER,
    //         },
    //         writer_sn: SequenceNumberSubmessageElement { value: 1 },
    //         inline_qos: ParameterListSubmessageElement::<&[Parameter]> { parameter: &[] },
    //         serialized_payload: SerializedDataSubmessageElement { value: &[4, 5, 6] },
    //     };

    //     const GAP_SUBMESSAGE1: GapSubmessage<Vec<SequenceNumber>> = GapSubmessage {
    //         endianness_flag: true,
    //         reader_id: EntityIdSubmessageElement {
    //             value: ENTITYID_UNKNOWN,
    //         },
    //         writer_id: EntityIdSubmessageElement {
    //             value: ENTITYID_UNKNOWN,
    //         },
    //         gap_start: SequenceNumberSubmessageElement { value: 1 },
    //         gap_list: SequenceNumberSetSubmessageElement {
    //             base: 1,
    //             set: vec![],
    //         },
    //     };

    //     impl<'a> StatelessWriterBehavior<'a, Vec<SequenceNumber>> for MockBehavior {
    //         type ReaderLocator = MockReaderLocator;

    //         fn send_unsent_data(
    //             &'a mut self,
    //             mut send_data: impl FnMut(&Self::ReaderLocator, DataSubmessage<'a, &'a [Parameter<'a>]>),
    //             mut send_gap: impl FnMut(&Self::ReaderLocator, GapSubmessage<Vec<SequenceNumber>>),
    //         ) {
    //             let locator1 = Locator::new(0, 1, [0; 16]);
    //             let locator2 = Locator::new(0, 2, [0; 16]);
    //             send_data(&MockReaderLocator(locator1), DATA_SUBMESSAGE1);
    //             send_data(&MockReaderLocator(locator1), DATA_SUBMESSAGE2);

    //             send_data(&MockReaderLocator(locator2), DATA_SUBMESSAGE2);
    //             send_gap(&MockReaderLocator(locator1), GAP_SUBMESSAGE1);

    //             send_gap(&MockReaderLocator(locator2), GAP_SUBMESSAGE1);
    //         }
    //     }

    //     let mut writer = MockBehavior;
    //     let destined_submessages: Vec<(Locator, Vec<RtpsSubmessageWrite>)> =
    //         writer.create_submessages();

    //     let locator1_submessages = &destined_submessages[0].1;
    //     let locator2_submessages = &destined_submessages[1].1;

    //     assert_eq!(destined_submessages.len(), 2);

    //     assert_eq!(locator1_submessages.len(), 3);
    //     assert_eq!(
    //         locator1_submessages[0],
    //         RtpsSubmessageType::Data(DATA_SUBMESSAGE1)
    //     );
    //     assert_eq!(
    //         locator1_submessages[1],
    //         RtpsSubmessageType::Data(DATA_SUBMESSAGE2)
    //     );
    //     assert_eq!(
    //         locator1_submessages[2],
    //         RtpsSubmessageType::Gap(GAP_SUBMESSAGE1)
    //     );

    //     assert_eq!(locator2_submessages.len(), 2);
    //     assert_eq!(
    //         locator2_submessages[0],
    //         RtpsSubmessageType::Data(DATA_SUBMESSAGE2)
    //     );
    //     assert_eq!(
    //         locator2_submessages[1],
    //         RtpsSubmessageType::Gap(GAP_SUBMESSAGE1)
    //     );
    // }
}
