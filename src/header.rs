#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Encoding {
    Utf16 = 0,
    Utf8 = 1,
}

#[derive(Debug)]
pub struct Header {
    pub encoding: Encoding,
    pub extended_uv: u8,
    pub vertex_index_size: u64,
    pub texture_index_size: u64,
    pub material_index_size: u64,
    pub bone_index_size: u64,
    pub morph_index_size: u64,
    pub rigid_index_size: u64,
}
