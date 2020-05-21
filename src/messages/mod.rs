use std::convert::Into;

// pub mod helpers;

// mod ack_nack_submessage;
// mod data_frag_submessage;
mod data_submessage;
mod gap_submessage;
// mod heartbeat_frag_submessage;
mod heartbeat_submessage;
// mod info_destination_submessage;
// mod info_reply_submessage;
// mod info_source_submessage;
mod info_timestamp_submessage;
// mod nack_frag_submessage;

use num_derive::FromPrimitive;

use crate::serdes::{RtpsSerialize, RtpsDeserialize, RtpsCompose, RtpsParse, EndianessFlag, RtpsSerdesResult, RtpsSerdesError, PrimitiveSerdes, SizeCheckers};
// use helpers::{deserialize, MINIMUM_RTPS_MESSAGE_SIZE};

use crate::types::*;


// pub use ack_nack_submessage::AckNack;
// pub use data_frag_submessage::DataFrag;
pub use data_submessage::Data;
pub use data_submessage::Payload;
pub use gap_submessage::Gap;
// pub use heartbeat_frag_submessage::HeartbeatFrag;
pub use heartbeat_submessage::Heartbeat;
// pub use info_destination_submessage::InfoDst;
// pub use info_reply_submessage::InfoReply;
// pub use info_source_submessage::InfoSrc;
pub use info_timestamp_submessage::InfoTs;
// pub use nack_frag_submessage::NackFrag;

#[derive(Debug)]
pub enum RtpsMessageError {
    MessageTooSmall,
    InvalidHeader,
    RtpsMajorVersionUnsupported,
    RtpsMinorVersionUnsupported,
    InvalidSubmessageHeader,
    InvalidSubmessage,
    InvalidKeyAndDataFlagCombination,
    CdrError(cdr::Error),
    IoError(std::io::Error),
    SerdesError(RtpsSerdesError),
    InvalidTypeConversion,
    DeserializationMessageSizeTooSmall,
}

impl From<cdr::Error> for RtpsMessageError {
    fn from(error: cdr::Error) -> Self {
        RtpsMessageError::CdrError(error)
    }
}

impl From<RtpsSerdesError> for RtpsMessageError {
    fn from(error: RtpsSerdesError) -> Self {
        RtpsMessageError::SerdesError(error)
    }
}

pub type RtpsMessageResult<T> = std::result::Result<T, RtpsMessageError>;

pub const RTPS_MAJOR_VERSION: u8 = 2;
pub const RTPS_MINOR_VERSION: u8 = 4;


#[derive(Debug, PartialEq)]
pub enum RtpsSubmessage {
    // AckNack(AckNack),
    Data(Data),
    // DataFrag(DataFrag),
    Gap(Gap),
    Heartbeat(Heartbeat),
    // HeartbeatFrag(HeartbeatFrag),
    // InfoDst(InfoDst),
    // InfoReply(InfoReply),
    // InfoSrc(InfoSrc),
    InfoTs(InfoTs),
    // NackFrag(NackFrag),
}

#[derive(FromPrimitive, PartialEq, Copy, Clone, Debug)]
pub enum SubmessageKind {
    Pad = 0x01,
    AckNack = 0x06,
    Heartbeat = 0x07,
    Gap = 0x08,
    InfoTimestamp = 0x09,
    InfoSource = 0x0c,
    InfoReplyIP4 = 0x0d,
    InfoDestination = 0x0e,
    InfoReply = 0x0f,
    NackFrag = 0x12,
    HeartbeatFrag = 0x13,
    Data = 0x15,
    DataFrag = 0x16,
}

impl RtpsSerialize for SubmessageKind
{
    fn serialize(&self, writer: &mut impl std::io::Write, _endianness: EndianessFlag) -> RtpsSerdesResult<()>{
        let submessage_kind_u8 = *self as u8;
        writer.write(&[submessage_kind_u8])?;

        Ok(())
    }
}

impl RtpsDeserialize for SubmessageKind
{
    fn deserialize(bytes: &[u8], _endianness: EndianessFlag) -> RtpsSerdesResult<Self> { 
        SizeCheckers::check_size_equal(bytes, 1 /*expected_size*/)?;
        Ok(num::FromPrimitive::from_u8(bytes[0]).ok_or(RtpsSerdesError::InvalidEnumRepresentation)?)
    }
}

impl RtpsParse for SubmessageKind {
    fn parse(bytes: &[u8]) -> RtpsSerdesResult<Self> {
        SizeCheckers::check_size_equal(bytes, 1 /*expected_size*/)?;
        Ok(num::FromPrimitive::from_u8(bytes[0]).ok_or(RtpsSerdesError::InvalidEnumRepresentation)?)
    }
}

struct OctetsToNextHeader(u16);

impl RtpsSerialize for OctetsToNextHeader
{
    fn serialize(&self, writer: &mut impl std::io::Write, endianness: EndianessFlag) -> RtpsSerdesResult<()> {
        writer.write(&PrimitiveSerdes::serialize_u16(self.0, endianness))?;

        Ok(())
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct SubmessageFlag(pub bool);

impl SubmessageFlag {
    pub fn is_set(&self) -> bool {
         self.0
    }
}

impl From<EndianessFlag> for SubmessageFlag {
    fn from(value: EndianessFlag) -> Self {
        SubmessageFlag(value.into())
    }
}

impl From<SubmessageFlag> for EndianessFlag {
    fn from(value: SubmessageFlag) -> Self {
        EndianessFlag::from(value.is_set())
    }
}

impl RtpsSerialize for [SubmessageFlag; 8] {
    fn serialize(&self, writer: &mut impl std::io::Write, _endianness: EndianessFlag) -> RtpsSerdesResult<()>{
        let mut flags = 0u8;
        for i in 0..8 {
            if self[i].0 {
                flags |= 0b00000001 << i;
            }
        }
        writer.write(&[flags])?;
        Ok(())
    }
}

impl RtpsParse for [SubmessageFlag; 8] {
    fn parse(bytes: &[u8]) -> RtpsSerdesResult<Self> { 
        // SizeCheckers::check_size_equal(bytes, 1)?;
        let flags: u8 = bytes[0];        
        let mut mask = 0b00000001_u8;
        let mut submessage_flags = [SubmessageFlag(false); 8];
        for i in 0..8 {
            if (flags & mask) > 0 {
                submessage_flags[i] = SubmessageFlag(true);
            }
            mask <<= 1;
        };
        Ok(submessage_flags)
    }
}

#[derive(PartialEq, Debug)]
pub struct SubmessageHeader {
    submessage_id: SubmessageKind,
    flags: [SubmessageFlag; 8],
    submessage_length: Ushort,
}

impl SubmessageHeader {
    pub fn submessage_id(&self) -> SubmessageKind {self.submessage_id}
    pub fn flags(&self) -> &[SubmessageFlag; 8] {&self.flags}
    pub fn submessage_length(&self) -> Ushort {self.submessage_length}
}

impl RtpsCompose for SubmessageHeader {
    fn compose(&self, writer: &mut impl std::io::Write) -> RtpsSerdesResult<()> {
        let endianness = EndianessFlag::from(self.flags[0].is_set());
        self.submessage_id.serialize(writer, endianness)?;
        self.flags.serialize(writer, endianness)?;
        self.submessage_length.serialize(writer, endianness)?;
        Ok(())
    }
}

impl RtpsParse for SubmessageHeader {
    fn parse(bytes: &[u8]) -> RtpsSerdesResult<Self> {   
        let submessage_id = SubmessageKind::parse(&bytes[0..1])?;
        let flags = <[SubmessageFlag; 8]>::parse(&bytes[1..2])?;
        let endianness = EndianessFlag::from(flags[0].is_set());
        let submessage_length = Ushort::deserialize(&bytes[2..4], endianness)?;
        Ok(SubmessageHeader {
            submessage_id, 
            flags,
            submessage_length,
        })
    }
}

pub trait Submessage {
    fn submessage_header(&self) -> SubmessageHeader;
}

// #[derive(Serialize, Deserialize, PartialEq, Debug)]
// struct MessageHeader {
//     protocol_name: [char; 4],
//     protocol_version: ProtocolVersion,
//     vendor_id: VendorId,
//     guid_prefix: GuidPrefix,
// }

#[derive(Debug)]
pub struct RtpsMessage {
    guid_prefix: GuidPrefix,
    vendor_id: VendorId,
    protocol_version: ProtocolVersion,
    submessages: Vec<RtpsSubmessage>,
}

impl RtpsMessage {
    pub fn new(
        guid_prefix: GuidPrefix,
        vendor_id: VendorId,
        protocol_version: ProtocolVersion,
    ) -> RtpsMessage {
        RtpsMessage {
            guid_prefix,
            vendor_id,
            protocol_version,
            submessages: Vec::new(),
        }
    }

    pub fn get_guid_prefix(&self) -> &GuidPrefix {
        &self.guid_prefix
    }

    pub fn get_vendor_id(&self) -> &VendorId {
        &self.vendor_id
    }

    pub fn get_protocol_version(&self) -> &ProtocolVersion {
        &self.protocol_version
    }

    pub fn push(&mut self, submessage: RtpsSubmessage) {
        self.submessages.push(submessage);
    }

    pub fn get_submessages(&self) -> &Vec<RtpsSubmessage> {
        &self.submessages
    }

    pub fn get_mut_submessages(&mut self) -> &mut Vec<RtpsSubmessage> {
        &mut self.submessages
    }

    pub fn take(
        self,
    ) -> (
        GuidPrefix,
        VendorId,
        ProtocolVersion,
        Vec<RtpsSubmessage>,
    ) {
        (
            self.guid_prefix,
            self.vendor_id,
            self.protocol_version,
            self.submessages,
        )
    }
}

// pub fn parse_rtps_message(message: &[u8]) -> RtpsMessageResult<RtpsMessage> {
//     const MESSAGE_HEADER_FIRST_INDEX: usize = 0;
//     const MESSAGE_HEADER_LAST_INDEX: usize = 19;
//     const PROTOCOL_VERSION_FIRST_INDEX: usize = 4;
//     const PROTOCOL_VERSION_LAST_INDEX: usize = 5;

//     if message.len() < MINIMUM_RTPS_MESSAGE_SIZE {
//         return Err(RtpsMessageError::MessageTooSmall);
//     }

//     let message_header = deserialize::<MessageHeader>(
//         message,
//         &MESSAGE_HEADER_FIRST_INDEX,
//         &MESSAGE_HEADER_LAST_INDEX,
//         &EndianessFlag::BigEndian, /* Endianness not relevant for the header. Only octets */
//     )?;

//     if message_header.protocol_name[0] != 'R'
//         || message_header.protocol_name[1] != 'T'
//         || message_header.protocol_name[2] != 'P'
//         || message_header.protocol_name[3] != 'S'
//     {
//         return Err(RtpsMessageError::InvalidHeader);
//     }

//     if message_header.protocol_version.major != 2 {
//         return Err(RtpsMessageError::RtpsMajorVersionUnsupported);
//     }
//     if message_header.protocol_version.minor > RTPS_MINOR_VERSION {
//         return Err(RtpsMessageError::RtpsMinorVersionUnsupported);
//     }

//     const RTPS_SUBMESSAGE_HEADER_SIZE: usize = 4;

//     let mut submessage_vector = Vec::with_capacity(4);

//     let mut submessage_first_index = MINIMUM_RTPS_MESSAGE_SIZE;
//     while submessage_first_index < message.len() {
//         const SUBMESSAGE_FLAGS_INDEX_OFFSET: usize = 1;

//         let submessage_header_first_index = submessage_first_index;
//         //In the deserialize library the comparisons are always inclusive of last element (-1 is required)
//         let submessage_header_last_index =
//             submessage_header_first_index + RTPS_SUBMESSAGE_HEADER_SIZE - 1;

//         if submessage_header_last_index >= message.len() {
//             return Err(RtpsMessageError::InvalidSubmessageHeader);
//         }

//         let submessage_endianess =
//             endianess(&message[submessage_header_first_index + SUBMESSAGE_FLAGS_INDEX_OFFSET])?;

//         let submessage_header = deserialize::<SubmessageHeader>(
//             message,
//             &submessage_header_first_index,
//             &submessage_header_last_index,
//             &submessage_endianess,
//         )?;

//         let submessage_payload_first_index = submessage_header_last_index + 1;
//         let submessage_payload_last_index = if submessage_header.submessage_length == 0 {
//             message.len() - 1
//         } else {
//             submessage_payload_first_index + submessage_header.submessage_length as usize - 1
//         };

//         if submessage_payload_last_index >= message.len() {
//             return Err(RtpsMessageError::MessageTooSmall); // TODO: Replace error by invalid message
//         }

//         let submessage = match num::FromPrimitive::from_u8(submessage_header.submessage_id)
//             .ok_or(RtpsMessageError::InvalidSubmessageHeader)?
//         {
//             SubmessageKind::AckNack => {
//                 RtpsSubmessage::AckNack(parse_ack_nack_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::Data => RtpsSubmessage::Data(parse_data_submessage(
//                 &message[submessage_payload_first_index..=submessage_payload_last_index],
//                 &submessage_header.flags,
//             )?),
//             SubmessageKind::DataFrag => {
//                 RtpsSubmessage::DataFrag(parse_data_frag_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::Gap => RtpsSubmessage::Gap(parse_gap_submessage(
//                 &message[submessage_payload_first_index..=submessage_payload_last_index],
//                 &submessage_header.flags,
//             )?),
//             SubmessageKind::Heartbeat => {
//                 RtpsSubmessage::Heartbeat(parse_heartbeat_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::HeartbeatFrag => {
//                 RtpsSubmessage::HeartbeatFrag(parse_heartbeat_frag_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::InfoDestination => {
//                 RtpsSubmessage::InfoDst(parse_info_dst_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::InfoReply => {
//                 RtpsSubmessage::InfoReply(parse_info_reply_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::InfoSource => {
//                 RtpsSubmessage::InfoSrc(parse_info_source_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::InfoTimestamp => {
//                 RtpsSubmessage::InfoTs(parse_info_timestamp_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::Pad => unimplemented!(),
//             SubmessageKind::NackFrag => {
//                 RtpsSubmessage::NackFrag(parse_nack_frag_submessage(
//                     &message[submessage_payload_first_index..=submessage_payload_last_index],
//                     &submessage_header.flags,
//                 )?)
//             }
//             SubmessageKind::InfoReplyIP4 => unimplemented!(),
//         };

//         submessage_vector.push(submessage);

//         submessage_first_index = submessage_payload_last_index + 1;
//     }

//     Ok(RtpsMessage {
//         guid_prefix: message_header.guid_prefix,
//         vendor_id: message_header.vendor_id,
//         protocol_version: message_header.protocol_version,
//         submessages: submessage_vector,
//     })
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtps_deserialize_for_submessage_flags() {
        let f = SubmessageFlag(false);
        let t = SubmessageFlag(true);

        let expected: [SubmessageFlag; 8] = [t, f, f, f, f, f, f, f];
        let bytes = [0b00000001_u8];    
        let result = <[SubmessageFlag; 8]>::parse(&bytes).unwrap();
        assert_eq!(expected, result);

        let expected: [SubmessageFlag; 8] = [t, t, f, t, f, f, f, f];
        let bytes = [0b00001011_u8];    
        let result = <[SubmessageFlag; 8]>::parse(&bytes).unwrap();
        assert_eq!(expected, result);

        let expected: [SubmessageFlag; 8] = [t, t, t, t, t, t, t, t];
        let bytes = [0b11111111_u8];    
        let result = <[SubmessageFlag; 8]>::parse(&bytes).unwrap();
        assert_eq!(expected, result);

        let expected: [SubmessageFlag; 8] = [f, f, f, f, f, f, f, f];
        let bytes = [0b00000000_u8];    
        let result = <[SubmessageFlag; 8]>::parse(&bytes).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_rtps_serialize_for_submessage_flags() {
        let f = SubmessageFlag(false);
        let t = SubmessageFlag(true);
        let mut writer = Vec::new();

        writer.clear();
        let flags: [SubmessageFlag; 8] = [t, f, f, f, f, f, f, f];
        flags.serialize(&mut writer, EndianessFlag::LittleEndian).unwrap();
        assert_eq!(writer, vec![0b00000001]);
        
        writer.clear();
        let flags: [SubmessageFlag; 8] = [f; 8];
        flags.serialize(&mut writer, EndianessFlag::LittleEndian).unwrap();
        assert_eq!(writer, vec![0b00000000]);
        
        writer.clear();
        let flags: [SubmessageFlag; 8] = [t; 8];
        flags.serialize(&mut writer, EndianessFlag::LittleEndian).unwrap();
        assert_eq!(writer, vec![0b11111111]);
        
        writer.clear();
        let flags: [SubmessageFlag; 8] = [f, t, f, f, t, t, f, t];
        flags.serialize(&mut writer, EndianessFlag::LittleEndian).unwrap();
        assert_eq!(writer, vec![0b10110010]);
    }

    #[test]
    fn test_deserialize_submessage_header_simple() {
        let bytes = [0x15_u8, 0b00000001, 20, 0x0];
        let f = SubmessageFlag(false);
        let flags: [SubmessageFlag; 8] = [SubmessageFlag(true), f, f, f, f, f, f, f];
        let expected = SubmessageHeader {
            submessage_id : SubmessageKind::Data, 
            flags,
            submessage_length: Ushort(20),
        };
        let result = SubmessageHeader::parse(&bytes);
    
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_rtps_serialize_for_submessage_header() {
        let mut result = Vec::new();

        let f = SubmessageFlag(false);
        let t = SubmessageFlag(true);
        let header = SubmessageHeader {
            submessage_id: SubmessageKind::Data,
            flags: [t, t, f, f, f, f, f, f],
            submessage_length: Ushort(16),
        };
        let expected = vec![0x15, 0b00000011, 16, 0x0];
        header.compose(&mut result).unwrap();
        assert_eq!(result, expected);
    }

    // #[test]
    // fn test_parse_valid_message_header_only() {
    //     let message_example = MessageHeader {
    //         protocol_name: ['R', 'T', 'P', 'S'],
    //         protocol_version: ProtocolVersion { major: 2, minor: 4 },
    //         vendor_id: [100, 210],
    //         guid_prefix: [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
    //     };

    //     let serialized =
    //         cdr::ser::serialize_data::<_, _, BigEndian>(&message_example, Infinite).unwrap();

    //     let parse_result = parse_rtps_message(&serialized).unwrap();

    //     assert_eq!(
    //         parse_result.guid_prefix,
    //         [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21]
    //     );
    //     assert_eq!(parse_result.submessages, vec!());
    // }

    // #[test]
    // fn test_parse_too_small_message() {
    //     let serialized = [0, 1, 2, 3];

    //     let parse_result = parse_rtps_message(&serialized);

    //     if let Err(RtpsMessageError::MessageTooSmall) = parse_result {
    //         assert!(true);
    //     } else {
    //         assert!(false);
    //     }
    // }

    // #[test]
    // fn test_parse_unsupported_version_header() {
    //     // Unsupported major version
    //     let message_example = MessageHeader {
    //         protocol_name: ['R', 'T', 'P', 'S'],
    //         protocol_version: ProtocolVersion { major: 1, minor: 4 },
    //         vendor_id: [100, 210],
    //         guid_prefix: [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
    //     };

    //     let serialized =
    //         cdr::ser::serialize_data::<_, _, BigEndian>(&message_example, Infinite).unwrap();

    //     let parse_result = parse_rtps_message(&serialized);

    //     if let Err(RtpsMessageError::RtpsMajorVersionUnsupported) = parse_result {
    //         assert!(true);
    //     } else {
    //         assert!(false);
    //     }

    //     // Unsupported minor version
    //     let message_example = MessageHeader {
    //         protocol_name: ['R', 'T', 'P', 'S'],
    //         protocol_version: ProtocolVersion { major: 2, minor: 5 },
    //         vendor_id: [100, 210],
    //         guid_prefix: [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
    //     };

    //     let serialized =
    //         cdr::ser::serialize_data::<_, _, BigEndian>(&message_example, Infinite).unwrap();

    //     let parse_result = parse_rtps_message(&serialized);

    //     if let Err(RtpsMessageError::RtpsMinorVersionUnsupported) = parse_result {
    //         assert!(true);
    //     } else {
    //         assert!(false);
    //     }

    //     // Unsupported major and minor version
    //     let message_example = MessageHeader {
    //         protocol_name: ['R', 'T', 'P', 'S'],
    //         protocol_version: ProtocolVersion {
    //             major: 3,
    //             minor: 10,
    //         },
    //         vendor_id: [100, 210],
    //         guid_prefix: [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
    //     };

    //     let serialized =
    //         cdr::ser::serialize_data::<_, _, BigEndian>(&message_example, Infinite).unwrap();

    //     let parse_result = parse_rtps_message(&serialized);

    //     if let Err(RtpsMessageError::RtpsMajorVersionUnsupported) = parse_result {
    //         assert!(true);
    //     } else {
    //         assert!(false);
    //     }
        
    // }

    // #[test]
    // fn test_parse_different_rtps_messages() {
    //     let rtps_message_info_ts_and_data = [
    //         0x52, 0x54, 0x50, 0x53, 0x02, 0x01, 0x01, 0x02, 0x7f, 0x20, 0xf7, 0xd7, 0x00, 0x00,
    //         0x01, 0xbb, 0x00, 0x00, 0x00, 0x01, 0x09, 0x01, 0x08, 0x00, 0x9e, 0x81, 0xbc, 0x5d,
    //         0x97, 0xde, 0x48, 0x26, 0x15, 0x07, 0x1c, 0x01, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
    //         0x00, 0x00, 0x00, 0x01, 0x00, 0xc2, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    //         0x70, 0x00, 0x10, 0x00, 0x7f, 0x20, 0xf7, 0xd7, 0x00, 0x00, 0x01, 0xbb, 0x00, 0x00,
    //         0x00, 0x01, 0x00, 0x00, 0x01, 0xc1, 0x01, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00,
    //         0x15, 0x00, 0x04, 0x00, 0x02, 0x01, 0x00, 0x00, 0x16, 0x00, 0x04, 0x00, 0x01, 0x02,
    //         0x00, 0x00, 0x31, 0x00, 0x18, 0x00, 0x01, 0x00, 0x00, 0x00, 0xf3, 0x1c, 0x00, 0x00,
    //         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0xa8,
    //         0x02, 0x04, 0x32, 0x00, 0x18, 0x00, 0x01, 0x00, 0x00, 0x00, 0xf2, 0x1c, 0x00, 0x00,
    //         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0xa8,
    //         0x02, 0x04, 0x02, 0x00, 0x08, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //         0x50, 0x00, 0x10, 0x00, 0x7f, 0x20, 0xf7, 0xd7, 0x00, 0x00, 0x01, 0xbb, 0x00, 0x00,
    //         0x00, 0x01, 0x00, 0x00, 0x01, 0xc1, 0x58, 0x00, 0x04, 0x00, 0x15, 0x04, 0x00, 0x00,
    //         0x00, 0x80, 0x04, 0x00, 0x15, 0x00, 0x00, 0x00, 0x07, 0x80, 0x5c, 0x00, 0x00, 0x00,
    //         0x00, 0x00, 0x2f, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //         0x50, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x44, 0x45, 0x53, 0x4b, 0x54, 0x4f,
    //         0x50, 0x2d, 0x4f, 0x52, 0x46, 0x44, 0x4f, 0x53, 0x35, 0x2f, 0x36, 0x2e, 0x31, 0x30,
    //         0x2e, 0x32, 0x2f, 0x63, 0x63, 0x36, 0x66, 0x62, 0x39, 0x61, 0x62, 0x33, 0x36, 0x2f,
    //         0x39, 0x30, 0x37, 0x65, 0x66, 0x66, 0x30, 0x32, 0x65, 0x33, 0x2f, 0x22, 0x78, 0x38,
    //         0x36, 0x5f, 0x36, 0x34, 0x2e, 0x77, 0x69, 0x6e, 0x2d, 0x76, 0x73, 0x32, 0x30, 0x31,
    //         0x35, 0x22, 0x2f, 0x00, 0x00, 0x00, 0x25, 0x80, 0x0c, 0x00, 0xd7, 0xf7, 0x20, 0x7f,
    //         0xbb, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    //     ];

    //     let parse_result = parse_rtps_message(&rtps_message_info_ts_and_data).unwrap();

    //     assert_eq!(
    //         parse_result.guid_prefix,
    //         [0x7f, 0x20, 0xf7, 0xd7, 0x00, 0x00, 0x01, 0xbb, 0x00, 0x00, 0x00, 0x01,]
    //     );
    //     assert_eq!(parse_result.submessages.len(), 2);
    //     if let RtpsSubmessage::InfoTs(ts_message) = &parse_result.submessages[0] {
    //         assert_eq!(
    //             *ts_message.get_timestamp(),
    //             Some(Time {
    //                 seconds: 1572635038,
    //                 fraction: 642309783,
    //             })
    //         );
    //     } else {
    //         assert!(false);
    //     }

    //     if let RtpsSubmessage::Data(data_message) = &parse_result.submessages[1] {
    //         assert_eq!(*data_message.reader_id(), EntityId::new([0, 0, 0], 0));
    //         assert_eq!(*data_message.writer_id(), EntityId::new([0, 1, 0], 0xc2)); //ENTITYID_SPDP_BUILTIN_PARTICIPANT_ANNOUNCER = {{00,01,00},c2}
    //         assert_eq!(*data_message.writer_sn(), 1);
    //         assert_eq!(
    //             data_message.inline_qos().as_ref().unwrap()[0],
    //             InlineQosParameter::KeyHash([
    //                 127, 32, 247, 215, 0, 0, 1, 187, 0, 0, 0, 1, 0, 0, 1, 193
    //             ])
    //         );
    //         assert_eq!(
    //             *data_message.serialized_payload(),
    //             Payload::Data(vec!(
    //                 0, 3, 0, 0, 21, 0, 4, 0, 2, 1, 0, 0, 22, 0, 4, 0, 1, 2, 0, 0, 49, 0, 24, 0, 1,
    //                 0, 0, 0, 243, 28, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 192, 168, 2, 4, 50,
    //                 0, 24, 0, 1, 0, 0, 0, 242, 28, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 192,
    //                 168, 2, 4, 2, 0, 8, 0, 11, 0, 0, 0, 0, 0, 0, 0, 80, 0, 16, 0, 127, 32, 247,
    //                 215, 0, 0, 1, 187, 0, 0, 0, 1, 0, 0, 1, 193, 88, 0, 4, 0, 21, 4, 0, 0, 0, 128,
    //                 4, 0, 21, 0, 0, 0, 7, 128, 92, 0, 0, 0, 0, 0, 47, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0,
    //                 0, 80, 0, 0, 0, 66, 0, 0, 0, 68, 69, 83, 75, 84, 79, 80, 45, 79, 82, 70, 68,
    //                 79, 83, 53, 47, 54, 46, 49, 48, 46, 50, 47, 99, 99, 54, 102, 98, 57, 97, 98,
    //                 51, 54, 47, 57, 48, 55, 101, 102, 102, 48, 50, 101, 51, 47, 34, 120, 56, 54,
    //                 95, 54, 52, 46, 119, 105, 110, 45, 118, 115, 50, 48, 49, 53, 34, 47, 0, 0, 0,
    //                 37, 128, 12, 0, 215, 247, 32, 127, 187, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0
    //             ))
    //         );
    //     } else {
    //         assert!(false);
    //     }
    // }
}
