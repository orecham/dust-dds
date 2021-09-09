use std::io::Write;

use cdr::Serializer;

use crate::dds_type::Endianness;

pub trait MappingWriteByteOrdered {
    fn write_ordered<W: Write, E: Endianness>(
        &self,
        writer: W,
    ) -> std::result::Result<(), std::io::Error>;
}

impl<T> MappingWriteByteOrdered for T
where
    T: serde::Serialize,
{
    fn write_ordered<W: Write, E: Endianness>(
        &self,
        writer: W,
    ) -> std::result::Result<(), std::io::Error> {
        let mut serializer = Serializer::<_, E::Endianness>::new(writer);
        serde::Serialize::serialize(self, &mut serializer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }
}

#[derive(Debug, PartialEq)]
pub struct ParameterSerialize<T> {
    parameter_id: u16,
    value: T,
}

impl<T: serde::Serialize> ParameterSerialize<T> {
    pub fn new(parameter_id: u16, value: T) -> Self {
        Self {
            parameter_id,
            value,
        }
    }
}

impl<T: serde::Serialize> MappingWriteByteOrdered for ParameterSerialize<T> {
    fn write_ordered<W: Write, E: Endianness>(
        &self,
        mut writer: W,
    ) -> std::result::Result<(), std::io::Error> {
        let length_without_padding = (cdr::calc_serialized_size(&self.value) - 4) as i16;
        let padding: &[u8] = match length_without_padding % 4 {
            1 => &[0; 3],
            2 => &[0; 2],
            3 => &[0; 1],
            _ => &[],
        };
        let length = length_without_padding + padding.len() as i16;
        self.parameter_id.write_ordered::<_, E>(&mut writer)?;
        length.write_ordered::<_, E>(&mut writer)?;
        let mut serializer = cdr::Serializer::<_, E::Endianness>::new(&mut writer);
        self.value.serialize(&mut serializer).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, err.to_string())
        })?;
        writer.write_all(padding)
    }
}

const PID_SENTINEL: u16 = 1;

pub struct ParameterListSerialize(pub Vec<ParameterSerialize<Box<dyn erased_serde::Serialize>>>);
impl MappingWriteByteOrdered for ParameterListSerialize {
    fn write_ordered<W: Write, E: Endianness>(
        &self,
        mut writer: W,
    ) -> std::result::Result<(), std::io::Error> {
        writer.write(&E::REPRESENTATION_IDENTIFIER).unwrap();
        writer.write(&E::REPRESENTATION_OPTIONS).unwrap();
        for parameter_i in &self.0 {
            parameter_i.write_ordered::<_, E>(&mut writer).unwrap();
        }
        PID_SENTINEL.write_ordered::<_, E>(&mut writer)?;
        [0_u8, 0].write_ordered::<_, E>(&mut writer)?;
        Ok(())
    }
}
