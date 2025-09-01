pub mod generated {
    #![allow(missing_docs)]

    #[rustfmt::skip]
    #[path = "services.reshard.v1.rs"]
    pub mod reshard;

    pub const FILE_DESCRIPTOR_SET: &[u8] = std::include_bytes!("generated/descriptor.bin");
}

mod host;
