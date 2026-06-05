// SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Initialize a librime struct, mirroring the C macro `RIME_STRUCT`.
///
/// Zeroes the struct and sets `data_size = sizeof(T) - sizeof(data_size)`,
/// which is the versioning convention librime uses for ABI forward compatibility.
#[macro_export]
macro_rules! rime_struct {
    ($var:ident : $t:ty) => {
        let mut $var: $t = unsafe { std::mem::zeroed() };
        $var.data_size =
            (std::mem::size_of::<$t>() - std::mem::size_of_val(&$var.data_size)) as std::ffi::c_int;
    };
}
