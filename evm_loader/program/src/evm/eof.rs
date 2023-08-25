#![allow(clippy::cast_possible_truncation)]

use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

use super::Buffer;
use crate::error::{Error, Result};

pub const OFFSET_TYPES_KIND: usize = 3;
pub const OFFSET_CODE_KIND: usize = 6;

pub const EOF1_VERSION: u8 = 1;

pub const EOF_MAGIC: [u8; 2] = [0xef, 0x00];

const HEADER_LEN_WITHOUT_TERMINATOR: usize = 14;
const SECTION_LEN: usize = 3;

fn assert_eof_version_1(bytes: &Buffer) -> Result<()> {
    if 2 < bytes.len() && bytes[2] == EOF1_VERSION {
        return Ok(());
    }

    Err(Error::InvalidVersion(bytes[2]))
}

pub fn has_eof_magic(bytes: &Buffer) -> bool {
    EOF_MAGIC.len() <= bytes.len() && bytes.starts_with(&EOF_MAGIC)
}

/// `Container` is an EOF container object.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Container {
    pub types: Vec<FunctionMetadata>,
    pub code: Vec<Buffer>,
    pub data: Buffer,
}

/// `FunctionMetadata` is an EOF function signature.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionMetadata {
    pub input: u8,
    pub output: u8,
    pub max_stack_height: u16,
}

impl FunctionMetadata {
    pub const MAX_INPUT_ITEMS: u8 = 127;
    pub const MAX_OUTPUT_ITEMS: u8 = 127;
    pub const MAX_STACK_HEIGHT: u16 = 1023;

    pub fn assert_valid(&self, section_index: u8) -> Result<()> {
        if self.input > Self::MAX_INPUT_ITEMS {
            return Err(Error::TooManyInputs(section_index, self.input));
        }

        if self.output > Self::MAX_OUTPUT_ITEMS {
            return Err(Error::TooManyOutputs(section_index, self.input));
        }

        if self.max_stack_height > Self::MAX_STACK_HEIGHT {
            return Err(Error::TooLargeMaxStackHeight(section_index, self.input));
        }

        Ok(())
    }

    pub fn _to_bytes(&self) -> Vec<u8> {
        vec![
            self.input,
            self.output,
            (self.max_stack_height >> 8) as u8,
            (self.max_stack_height & 0x00ff) as u8,
        ]
    }
}

#[derive(Debug)]
pub struct Section {
    pub kind: SectionKind,
    pub size: u16,
}

impl Section {
    /// `parse_section` decodes a (kind, size) [pair][Section] from an EOF header.
    fn parse(bytes: &Buffer, idx: usize) -> Result<Self> {
        if idx + SECTION_LEN >= bytes.len() {
            return Err(Error::UnexpectedEndOfFile);
        }

        Ok(Section {
            kind: SectionKind::try_from(bytes[idx])?,
            size: bytes.get_u16_or_default(idx + 1),
        })
    }
}

#[derive(Debug)]
pub struct SectionList {
    pub kind: SectionKind,
    pub list: Vec<u16>,
}

impl SectionList {
    /// `parse_section_list` decodes a (kind, len, []codeSize) [section list][SectionList] from an EOF header.
    fn parse(bytes: &Buffer, idx: usize) -> Result<Self> {
        if idx >= bytes.len() {
            return Err(Error::UnexpectedEndOfFile);
        }

        Ok(SectionList {
            kind: SectionKind::try_from(bytes[idx])?,
            list: Self::parse_list(bytes, idx + 1)?,
        })
    }

    // `parse_list` decodes a list of u16..
    fn parse_list(bytes: &Buffer, idx: usize) -> Result<Vec<u16>> {
        if bytes.len() < idx + 2 {
            return Err(Error::UnexpectedEndOfFile);
        }

        let count = bytes.get_u16_or_default(idx);

        if bytes.len() <= idx + 2 + (count as usize) * 2 {
            return Err(Error::UnexpectedEndOfFile);
        }

        Ok((0..(count as usize))
            .map(|i| bytes.get_u16_or_default(idx + 2 + 2 * i))
            .collect::<Vec<_>>())
    }

    pub fn total_size(&self) -> u16 {
        self.list.iter().sum::<_>()
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SectionKind {
    Type = 1,
    Code,
    Data,
}

impl SectionKind {
    pub fn assert(&self, expected: SectionKind) -> Result<()> {
        if !self.eq(&expected) {
            return Err(Error::MissingSectionHeader(
                expected as u8,
                self.clone() as u8,
            ));
        }

        Ok(())
    }
}

impl TryFrom<u8> for SectionKind {
    type Error = Error;

    fn try_from(kind: u8) -> Result<Self> {
        match kind {
            x if x == SectionKind::Type as u8 => Ok(SectionKind::Type),
            x if x == SectionKind::Code as u8 => Ok(SectionKind::Code),
            x if x == SectionKind::Data as u8 => Ok(SectionKind::Data),
            _ => Err(Error::UnknownSectionHeader(kind)),
        }
    }
}

impl Container {
    /// `marshal_binary` encodes an EOF [Container] into binary format.
    pub fn _marshal_binary(&self) -> Buffer {
        let mut bytes = Vec::from(EOF_MAGIC);

        bytes.push(EOF1_VERSION);

        // type section
        bytes.push(SectionKind::Type as u8);
        bytes.extend(((self.types.len() * 4) as u16).to_be_bytes());

        // code section
        bytes.push(SectionKind::Code as u8);
        bytes.extend((self.code.len() as u16).to_be_bytes());

        bytes.extend(
            self.code
                .iter()
                .flat_map(|code| (code.len() as u16).to_be_bytes()),
        );

        // data section
        bytes.push(SectionKind::Data as u8);
        bytes.extend((self.data.len() as u16).to_be_bytes());

        // terminator
        bytes.push(0u8);

        // write section contents
        let type_section_content = self
            .types
            .iter()
            .flat_map(FunctionMetadata::_to_bytes)
            .collect::<Vec<_>>();

        bytes.extend(type_section_content);

        bytes.extend(self.code.iter().flat_map(|buf| buf.to_vec()));
        bytes.extend(self.data.to_vec());

        Buffer::from_slice(&bytes)
    }

    /// `unmarshal_binary` decodes an EOF [Container].
    pub fn unmarshal_binary(bytes: &Buffer) -> Result<Self> {
        if !has_eof_magic(bytes) {
            return Err(Error::InvalidMagic);
        }

        if bytes.len() < HEADER_LEN_WITHOUT_TERMINATOR {
            return Err(Error::UnexpectedEndOfFile);
        }

        assert_eof_version_1(bytes)?;

        let type_section = Section::parse(bytes, OFFSET_TYPES_KIND)?;

        type_section.kind.assert(SectionKind::Type)?;

        if type_section.size < 4 || type_section.size % 4 != 0 {
            return Err(Error::InvalidTypeSizeMustBeDivisibleBy4(type_section.size));
        }

        let type_section_size = (type_section.size / 4) as usize;

        if type_section_size > 1024 {
            return Err(Error::InvalidTypeSizeExceed(type_section.size));
        }

        let code_section_list = SectionList::parse(bytes, OFFSET_CODE_KIND)?;

        code_section_list.kind.assert(SectionKind::Code)?;

        if code_section_list.list.len() != type_section_size {
            return Err(Error::MismatchCodeSize(
                type_section_size,
                code_section_list.list.len(),
            ));
        }

        let offset_data_kind = OFFSET_CODE_KIND + 2 + 2 * code_section_list.list.len() + 1;
        let data_section = Section::parse(bytes, offset_data_kind)?;

        data_section.kind.assert(SectionKind::Data)?;

        let offset_terminator = offset_data_kind + 3;

        match bytes.get(offset_terminator) {
            None => return Err(Error::UnexpectedEndOfFile),
            Some(terminator) => {
                if *terminator != 0 {
                    return Err(Error::MissingTerminator(*terminator));
                }
            }
        }

        let expected_size = ((offset_terminator as u16)
            + type_section.size
            + code_section_list.total_size()
            + data_section.size
            + 1) as usize;

        if bytes.len() != expected_size {
            return Err(Error::InvalidContainerSize(bytes.len(), expected_size));
        }

        let idx = offset_terminator + 1;

        let types = (0..type_section_size)
            .map(|section_index| {
                let signature = FunctionMetadata {
                    input: bytes[idx + section_index * 4],
                    output: bytes[idx + section_index * 4 + 1],
                    max_stack_height: bytes.get_u16_or_default(idx + section_index * 4 + 2),
                };

                signature.assert_valid(section_index as u8)?;

                Ok(signature)
            })
            .collect::<Result<Vec<_>>>()?;

        if types[0].input != 0 || types[0].output != 0 {
            return Err(Error::InvalidSection0Type(types[0].input, types[0].output));
        }

        let mut idx = idx + (type_section.size as usize);
        let mut code: Vec<Vec<u8>> = Vec::with_capacity(code_section_list.list.len());

        for i in 0..code_section_list.list.len() {
            let size = code_section_list.list[i] as usize;

            if size == 0 {
                return Err(Error::InvalidCodeSize(i));
            }

            code.push(Vec::from(&bytes[idx..idx + size]));
            idx += size;
        }

        let data = Buffer::from_slice(&bytes[idx..idx + (data_section.size as usize)]);

        let code = code
            .iter()
            .map(|v| Buffer::from_slice(v))
            .collect::<Vec<_>>();

        Ok(Container { types, code, data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rustfmt::skip]
    const BYTES_WITH_EMPTY_DATA: &[u8] = &[239u8, 0, 1, 1, 0, 12, 2, 0, 3, 0, 3, 0, 5, 0, 1, 3, 0, 0, 0, 0, 0, 0, 1, 2, 3, 0, 4, 1, 1, 0, 1, 96, 66, 0, 96, 66, 96, 66, 0, 0];
    #[rustfmt::skip]
    const BYTES_WITH_DATA: &[u8]= &[239u8, 0, 1, 1, 0, 4, 2, 0, 1, 0, 3, 3, 0, 3, 0, 0, 0, 0, 1, 96, 66, 0, 1, 2, 3];

    fn get_container_with_data() -> Container {
        Container {
            types: vec![FunctionMetadata {
                input: 0,
                output: 0,
                max_stack_height: 1,
            }],
            code: vec![Buffer::from_slice(&hex::decode("604200").unwrap())],
            data: Buffer::from_slice(&[0x01, 0x02, 0x03]),
        }
    }

    fn get_container_with_empty_data() -> Container {
        Container {
            types: vec![
                FunctionMetadata {
                    input: 0,
                    output: 0,
                    max_stack_height: 1,
                },
                FunctionMetadata {
                    input: 2,
                    output: 3,
                    max_stack_height: 4,
                },
                FunctionMetadata {
                    input: 1,
                    output: 1,
                    max_stack_height: 1,
                },
            ],
            code: vec![
                Buffer::from_slice(&hex::decode("604200").unwrap()),
                Buffer::from_slice(&hex::decode("6042604200").unwrap()),
                Buffer::from_slice(&hex::decode("00").unwrap()),
            ],
            data: Buffer::empty(),
        }
    }

    #[test]
    fn marshal_binary_with_empty_data() {
        let expected_bytes = Vec::from(BYTES_WITH_EMPTY_DATA);
        let container = get_container_with_empty_data();

        let bytes = container._marshal_binary().to_vec();

        assert_eq!(bytes.len(), expected_bytes.len());
        assert_eq!(bytes, expected_bytes);
    }

    #[test]
    fn marshal_binary_with_data() {
        let expected_bytes = Vec::from(BYTES_WITH_DATA);
        let container = get_container_with_data();

        let bytes = container._marshal_binary().to_vec();

        assert_eq!(bytes.len(), expected_bytes.len());
        assert_eq!(bytes, expected_bytes);
    }

    #[test]
    fn unmarshal_binary_with_data() {
        let bytes = Buffer::from_slice(BYTES_WITH_DATA);
        let expected_container = get_container_with_data();

        let container = Container::unmarshal_binary(&bytes).unwrap();

        assert_eq!(container, expected_container);
    }

    #[test]
    fn unmarshal_binary_with_empty_data() {
        let bytes = Buffer::from_slice(BYTES_WITH_EMPTY_DATA);
        let expected_container = get_container_with_empty_data();

        let container = Container::unmarshal_binary(&bytes).unwrap();

        assert_eq!(container, expected_container);
    }

    #[test]
    #[should_panic(expected = "InvalidMagic")]
    fn unmarshal_binary_without_magic() {
        let bytes = Buffer::from_slice(&BYTES_WITH_EMPTY_DATA[2..]);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "UnexpectedEndOfFile")]
    fn unmarshal_binary_when_lenght_is_less_than_14() {
        let bytes = Buffer::from_slice(&BYTES_WITH_EMPTY_DATA[..6]);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidVersion")]
    fn unmarshal_binary_with_invalid_version() {
        let mut bytes = Vec::from(BYTES_WITH_DATA);

        bytes[2] = 100;

        let bytes = Buffer::from_slice(&bytes);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "UnknownSectionHeader(15)")]
    fn unmarshal_binary_with_unknown_section_header_for_type_section() {
        let mut bytes = Vec::from(BYTES_WITH_DATA);
        // type section kind
        bytes[3] = 15;

        let bytes = Buffer::from_slice(&bytes);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "MissingSectionHeader(1, 3)")]
    fn unmarshal_binary_with_wrong_section_header_for_type_section() {
        let mut bytes = Vec::from(BYTES_WITH_DATA);
        // type section kind
        bytes[3] = SectionKind::Data as u8;

        let bytes = Buffer::from_slice(&bytes);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidTypeSizeMustBeDivisibleBy4(25)")]
    fn unmarshal_binary_with_invalid_types_size_must_be_divisible_by_4() {
        let mut bytes = Vec::from(BYTES_WITH_DATA);

        let new_size = 25u16.to_be_bytes();

        // type section size bytes
        bytes[4] = new_size[0];
        bytes[5] = new_size[1];

        let bytes = Buffer::from_slice(&bytes);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidTypeSizeExceed(5120)")]
    fn unmarshal_binary_with_invalid_type_size_exceed() {
        let mut bytes = Vec::from(BYTES_WITH_DATA);

        let new_size = (1024u16 * 5).to_be_bytes();
        // type section size bytes
        bytes[4] = new_size[0];
        bytes[5] = new_size[1];

        let bytes = Buffer::from_slice(&bytes);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "MismatchCodeSize(514, 1)")]
    fn unmarshal_binary_with_mismatched_code_size() {
        let mut bytes = Vec::from(BYTES_WITH_DATA);
        let new_size = (513u16 * 4 + 4).to_be_bytes();

        // type section size bytes
        bytes[4] = new_size[0];
        bytes[5] = new_size[1];

        let bytes = Buffer::from_slice(&bytes);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "MissingTerminator(1)")]
    fn unmarshal_binary_with_missing_terminator() {
        let mut bytes = Vec::from(BYTES_WITH_DATA);

        // terminator
        bytes[14] = 1;

        let bytes = Buffer::from_slice(&bytes);

        Container::unmarshal_binary(&bytes).unwrap();
    }

    #[test]
    #[should_panic(expected = "InvalidContainerSize(26, 25)")]
    fn unmarshal_binary_with_invalid_container_size() {
        let bytes = Buffer::from_slice(&[BYTES_WITH_DATA, &[5u8]].concat());

        Container::unmarshal_binary(&bytes).unwrap();
    }
}
