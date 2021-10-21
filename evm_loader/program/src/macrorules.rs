//! `EVM_LOADER` MACRO RULES
#![allow(clippy::use_self,clippy::nursery)]

macro_rules! str_as_bytes_len {
    ($value:expr) => {
        {
            $value.as_bytes().len()
        }
    }
}

macro_rules! neon_elf_param {
    ($identifier:ident,$value:expr) => {
        /// NEON DOCS MUST BE HERE
        #[no_mangle]
        #[used]
        pub static $identifier: [u8; str_as_bytes_len!($value)] = 
            {
                let mut array: [u8; str_as_bytes_len!($value)] = [0; str_as_bytes_len!($value)];
                let mut i = 0;
                while i < str_as_bytes_len!($value) {
                    array[i] = $value.as_bytes()[i];
                    i += 1;
                }
                array
            };
    }
}

macro_rules! declare_param_id {
    ($identifier:ident,$value:expr) => {
            solana_program::declare_id!($value);
            neon_elf_param!( $identifier, $value);
    }
}

pub(crate) use str_as_bytes_len;
pub(crate) use neon_elf_param;
pub(crate) use declare_param_id;
