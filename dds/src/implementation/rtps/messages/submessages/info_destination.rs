use crate::{
    implementation::rtps::{
        messages::{
            overall_structure::{
                Submessage, SubmessageHeaderRead, SubmessageHeaderWrite, TryReadFromBytes,
                WriteIntoBytes,
            },
            types::SubmessageKind,
        },
        types::GuidPrefix,
    },
    infrastructure::error::DdsResult,
};

#[derive(Debug, PartialEq, Eq)]
pub struct InfoDestinationSubmessageRead {
    guid_prefix: GuidPrefix,
}

impl InfoDestinationSubmessageRead {
    pub fn try_from_bytes(
        submessage_header: &SubmessageHeaderRead,
        mut data: &[u8],
    ) -> DdsResult<Self> {
        Ok(Self {
            guid_prefix: GuidPrefix::try_read_from_bytes(
                &mut data,
                submessage_header.endianness(),
            )?,
        })
    }

    pub fn guid_prefix(&self) -> GuidPrefix {
        self.guid_prefix
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct InfoDestinationSubmessageWrite {
    guid_prefix: GuidPrefix,
}

impl InfoDestinationSubmessageWrite {
    pub fn new(guid_prefix: GuidPrefix) -> Self {
        Self { guid_prefix }
    }
}

impl Submessage for InfoDestinationSubmessageWrite {
    fn write_submessage_header_into_bytes(&self, octets_to_next_header: u16, mut buf: &mut [u8]) {
        SubmessageHeaderWrite::new(SubmessageKind::INFO_DST, &[], octets_to_next_header)
            .write_into_bytes(&mut buf);
    }

    fn write_submessage_elements_into_bytes(&self, buf: &mut &mut [u8]) {
        self.guid_prefix.write_into_bytes(buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementation::rtps::{
        messages::overall_structure::write_into_bytes_vec, types::GUIDPREFIX_UNKNOWN,
    };

    #[test]
    fn serialize_heart_beat() {
        let guid_prefix = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let submessage = InfoDestinationSubmessageWrite::new(guid_prefix);
        #[rustfmt::skip]
        assert_eq!(write_into_bytes_vec(submessage), vec![
              0x0e, 0b_0000_0001, 12, 0, // Submessage header
                1, 2, 3, 4, //guid_prefix
                5, 6, 7, 8, //guid_prefix
                9, 10, 11, 12, //guid_prefix
            ]
        );
    }

    #[test]
    fn deserialize_info_destination() {
        #[rustfmt::skip]
        let mut data = &[
            0x0e, 0b_0000_0001, 12, 0, // Submessage header
            0, 0, 0, 0, //guid_prefix
            0, 0, 0, 0, //guid_prefix
            0, 0, 0, 0, //guid_prefix
        ][..];
        let submessage_header = SubmessageHeaderRead::try_read_from_bytes(&mut data).unwrap();
        let submessage =
            InfoDestinationSubmessageRead::try_from_bytes(&submessage_header, data).unwrap();

        let expected_guid_prefix = GUIDPREFIX_UNKNOWN;
        assert_eq!(expected_guid_prefix, submessage.guid_prefix());
    }
}
