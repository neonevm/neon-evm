//! `EVM_LOADER` MACRO RULES

macro_rules! neon_elf_param {
    ($identifier:ident, $value:expr) => {
        #[no_mangle]
        #[used]
        pub static $identifier: [u8; $value.len()] = {
            #[allow(clippy::string_lit_as_bytes)]
            let bytes: &[u8] = $value.as_bytes();

            let mut array = [0; $value.len()];
            let mut i = 0;
            while i < $value.len() {
                array[i] = bytes[i];
                i += 1;
            }
            array
        };
    }
}

macro_rules! declare_param_id {
    ($identifier:ident, $value:expr) => {
        ::solana_program::declare_id!($value);
        $crate::config_macro::neon_elf_param!($identifier, $value);
    }
}

macro_rules! pubkey_array {
    ($identifier:ident, [ $($value:expr,)* ]) => {
        pub static $identifier: [::solana_program::pubkey::Pubkey; [$($value,)*].len()] = [
            $(::solana_program::pubkey!($value),)*
        ];
    };
}

pub(crate) use neon_elf_param;
pub(crate) use declare_param_id;
pub(crate) use pubkey_array;
