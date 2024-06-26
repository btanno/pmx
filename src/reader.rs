use super::*;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported version")]
    UnsupportedVersion,
    #[error("invalid header: {0}")]
    InvalidHeader(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("io error: {0}")]
    Io(std::io::Error),
}

impl Error {
    pub(crate) fn invalid_header(msg: impl Into<String>) -> Self {
        Self::InvalidHeader(msg.into())
    }

    pub(crate) fn invalid_data(msg: impl Into<String>) -> Self {
        Self::InvalidData(msg.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

struct Seeker<'a> {
    reader: &'a mut Cursor<&'a Vec<u8>>,
}

impl<'a> Seeker<'a> {
    fn new(reader: &'a mut Cursor<&'a Vec<u8>>) -> Self {
        Self { reader }
    }

    fn position(&self) -> u64 {
        self.reader.position()
    }

    fn seek_bin(&mut self, len: u64) -> Result<u64, Error> {
        let first = self.position();
        self.reader.seek(SeekFrom::Current(len as i64))?;
        Ok(first)
    }

    fn seek_string(&mut self) -> Result<u64, Error> {
        let first = self.position();
        let mut buffer = [0u8; 4];
        self.reader.read_exact(&mut buffer)?;
        let len = u32::from_le_bytes(buffer);
        self.reader.seek(SeekFrom::Current(len as i64))?;
        Ok(first)
    }

    fn read_u8(&mut self) -> Result<u8, Error> {
        let mut buffer = [0u8; 1];
        self.reader.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }

    fn read_u16(&mut self) -> Result<u16, Error> {
        let mut buffer = [0u8; 2];
        self.reader.read_exact(&mut buffer)?;
        Ok(u16::from_le_bytes(buffer))
    }

    fn read_u32(&mut self) -> Result<u32, Error> {
        let mut buffer = [0u8; 4];
        self.reader.read_exact(&mut buffer)?;
        Ok(u32::from_le_bytes(buffer))
    }
}

struct Indices {
    name: u64,
    name_en: u64,
    comment: u64,
    comment_en: u64,
    vertices: u64,
    faces: u64,
    textures: u64,
    materials: u64,
    bones: u64,
    morphs: u64,
    display_groups: u64,
    rigids: u64,
    joints: u64,
}

impl Indices {
    fn new<'a>(reader: &'a mut Cursor<&'a Vec<u8>>, header: &Header) -> Result<Self, Error> {
        let mut seeker = Seeker::new(reader);
        let name = seeker.seek_string()?;
        let name_en = seeker.seek_string()?;
        let comment = seeker.seek_string()?;
        let comment_en = seeker.seek_string()?;
        let vertices = seeker.position();
        let vertices_len = seeker.read_u32()?;
        for _ in 0..vertices_len {
            seeker.seek_bin(4 * 3)?;
            seeker.seek_bin(4 * 3)?;
            seeker.seek_bin(4 * 2)?;
            seeker.seek_bin(4 * 4 * header.extended_uv as u64)?;
            match seeker.read_u8()? {
                0 => {
                    seeker.seek_bin(header.bone_index_size)?;
                }
                1 => {
                    seeker.seek_bin(header.bone_index_size * 2 + 4)?;
                }
                2 => {
                    seeker.seek_bin(header.bone_index_size * 4 + 4 * 4)?;
                }
                3 => {
                    seeker.seek_bin(header.bone_index_size * 2 + 4 + 4 * 3 * 3)?;
                }
                _ => return Err(Error::invalid_data("vertex weight type")),
            }
            seeker.seek_bin(4)?;
        }
        let faces = seeker.position();
        let faces_len = seeker.read_u32()?;
        if faces_len % 3 != 0 {
            return Err(Error::invalid_data("faces"));
        }
        seeker.seek_bin(header.vertex_index_size * faces_len as u64)?;
        let textures = seeker.position();
        let textures_len = seeker.read_u32()?;
        for _ in 0..textures_len {
            seeker.seek_string()?;
        }
        let materials = seeker.position();
        let materials_len = seeker.read_u32()?;
        for _ in 0..materials_len {
            seeker.seek_string()?;
            seeker.seek_string()?;
            seeker.seek_bin(16 + 12 + 4 + 12 + 1 + 16 + 4 + header.texture_index_size * 2)?;
            let sphere_mode = seeker.read_u8()?;
            if sphere_mode > 3 {
                return Err(Error::invalid_data("material sphere mode"));
            }
            match seeker.read_u8()? {
                0 => {
                    seeker.seek_bin(header.texture_index_size.into())?;
                }
                1 => {
                    seeker.seek_bin(1)?;
                }
                _ => return Err(Error::invalid_data("material toon flag")),
            }
            seeker.seek_string()?;
            seeker.seek_bin(4)?;
        }
        let bones = seeker.position();
        let bones_len = seeker.read_u32()?;
        for _ in 0..bones_len {
            seeker.seek_string()?;
            seeker.seek_string()?;
            seeker.seek_bin(12 + header.bone_index_size as u64 + 4)?;
            let flags = seeker.read_u16()?;
            if flags & 0x0001 == 0 {
                seeker.seek_bin(12)?;
            } else {
                seeker.seek_bin(header.bone_index_size)?;
            }
            if flags & 0x0100 != 0 || flags & 0x0200 != 0 {
                seeker.seek_bin(header.bone_index_size + 4)?;
            }
            if flags & 0x0400 != 0 {
                seeker.seek_bin(12)?;
            }
            if flags & 0x0800 != 0 {
                seeker.seek_bin(12 + 12)?;
            }
            if flags & 0x2000 != 0 {
                seeker.seek_bin(4)?;
            }
            if flags & 0x0020 != 0 {
                seeker.seek_bin(header.bone_index_size + 4 + 4)?;
                let link = seeker.read_u32()?;
                for _ in 0..link {
                    seeker.seek_bin(header.bone_index_size)?;
                    let angle_limit = seeker.read_u8()?;
                    if angle_limit == 1 {
                        seeker.seek_bin(12 + 12)?;
                    }
                }
            }
        }
        let morphs = seeker.position();
        let morphs_len = seeker.read_u32()?;
        for _ in 0..morphs_len {
            seeker.seek_string()?;
            seeker.seek_string()?;
            let panel = seeker.read_u8()?;
            if panel > 4 {
                return Err(Error::invalid_data("morph panel"));
            }
            let ty = seeker.read_u8()?;
            let len = seeker.read_u32()?;
            match ty {
                0 => {
                    for _ in 0..len {
                        seeker.seek_bin(header.morph_index_size + 4)?;
                    }
                }
                1 => {
                    for _ in 0..len {
                        seeker.seek_bin(header.vertex_index_size + 12)?;
                    }
                }
                2 => {
                    for _ in 0..len {
                        seeker.seek_bin(header.bone_index_size + 12 + 16)?;
                    }
                }
                3 | 4 | 5 | 6 | 7 => {
                    for _ in 0..len {
                        seeker.seek_bin(header.vertex_index_size + 16)?;
                    }
                }
                8 => {
                    for _ in 0..len {
                        seeker.seek_bin(header.material_index_size)?;
                        let op = seeker.read_u8()?;
                        if op > 1 {
                            return Err(Error::invalid_data("morph material op"));
                        }
                        seeker.seek_bin(16 + 12 + 4 + 12 + 16 + 4 + 16 + 16 + 16)?;
                    }
                }
                _ => return Err(Error::invalid_data("morph type")),
            }
        }
        let display_groups = seeker.position();
        let display_groups_len = seeker.read_u32()?;
        for _ in 0..display_groups_len {
            seeker.seek_string()?;
            seeker.seek_string()?;
            seeker.seek_bin(1)?;
            let len = seeker.read_u32()?;
            for _ in 0..len {
                let element = seeker.read_u8()?;
                match element {
                    0 => {
                        seeker.seek_bin(header.bone_index_size)?;
                    }
                    1 => {
                        seeker.seek_bin(header.morph_index_size)?;
                    }
                    _ => return Err(Error::invalid_data("display group element")),
                }
            }
        }
        let rigids = seeker.position();
        let rigids_len = seeker.read_u32()?;
        for _ in 0..rigids_len {
            seeker.seek_string()?;
            seeker.seek_string()?;
            seeker.seek_bin(header.bone_index_size + 1 + 2)?;
            let shape = seeker.read_u8()?;
            if shape > 2 {
                return Err(Error::invalid_data("rigid shape"));
            }
            seeker.seek_bin(12 + 12 + 12 + 4 + 4 + 4 + 4 + 4)?;
            let method = seeker.read_u8()?;
            if method > 2 {
                return Err(Error::invalid_data("rigid method"));
            }
        }
        let joints = seeker.position();
        let joints_len = seeker.read_u32()?;
        for _ in 0..joints_len {
            seeker.seek_string()?;
            seeker.seek_string()?;
            let ty = seeker.read_u8()?;
            if ty != 0 {
                return Err(Error::invalid_data("joint type"));
            }
            seeker.seek_bin(header.rigid_index_size * 2 + 12 * 2 + 12 * 4 + 12 * 2)?;
        }
        Ok(Self {
            name,
            name_en,
            comment,
            comment_en,
            vertices,
            faces,
            textures,
            materials,
            bones,
            morphs,
            display_groups,
            rigids,
            joints,
        })
    }
}

#[derive(Clone)]
struct DataCursor<'a> {
    reader: Cursor<&'a Vec<u8>>,
    header: &'a Header,
}

impl<'a> DataCursor<'a> {
    fn new(data: &'a Vec<u8>, header: &'a Header) -> Self {
        Self {
            reader: Cursor::new(data),
            header,
        }
    }

    fn with_position(data: &'a Vec<u8>, header: &'a Header, pos: SeekFrom) -> Self {
        let mut this = Self::new(data, header);
        this.seek(pos).unwrap();
        this
    }

    fn read_bin<const N: usize>(&mut self) -> [u8; N] {
        let mut buffer = [0u8; N];
        self.reader.read_exact(&mut buffer).unwrap();
        buffer
    }

    fn read_u8(&mut self) -> u8 {
        self.read_bin::<1>()[0]
    }

    fn read_u16(&mut self) -> u16 {
        u16::from_le_bytes(self.read_bin::<2>())
    }

    fn read_u32(&mut self) -> u32 {
        u32::from_le_bytes(self.read_bin::<4>())
    }

    fn read_i8(&mut self) -> i8 {
        i8::from_le_bytes(self.read_bin::<1>())
    }

    fn read_i16(&mut self) -> i16 {
        i16::from_le_bytes(self.read_bin::<2>())
    }

    fn read_i32(&mut self) -> i32 {
        i32::from_le_bytes(self.read_bin::<4>())
    }

    fn read_f32(&mut self) -> f32 {
        f32::from_le_bytes(self.read_bin::<4>())
    }

    fn read_vec<const N: usize>(&mut self) -> [f32; N] {
        let mut buffer = [0.0f32; N];
        let b = unsafe { std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, 4 * N) };
        self.reader.read_exact(b).unwrap();
        buffer
    }

    fn read_vec2(&mut self) -> [f32; 2] {
        self.read_vec::<2>()
    }

    fn read_vec3(&mut self) -> [f32; 3] {
        self.read_vec::<3>()
    }

    fn read_vec4(&mut self) -> [f32; 4] {
        self.read_vec::<4>()
    }

    fn read_string(&mut self) -> String {
        let len = self.read_u32() as usize;
        if len == 0 {
            return String::new();
        }
        let mut buffer = vec![0u8; len];
        self.reader.read_exact(&mut buffer).unwrap();
        match self.header.encoding {
            Encoding::Utf16 => unsafe {
                let buffer = std::slice::from_raw_parts(buffer.as_ptr() as *const u16, len / 2);
                String::from_utf16_lossy(buffer)
            },
            Encoding::Utf8 => String::from_utf8_lossy(&buffer).to_string(),
        }
    }

    fn read_signed_index(&mut self, size: u64) -> Option<usize> {
        let v = match size {
            1 => self.read_i8() as i32,
            2 => self.read_i16() as i32,
            4 => self.read_i32(),
            _ => unreachable!(),
        };
        (v >= 0).then_some(v as usize)
    }

    fn read_vertex_index(&mut self) -> usize {
        match self.header.vertex_index_size {
            1 => self.read_u8() as usize,
            2 => self.read_u16() as usize,
            4 => self.read_i32() as usize,
            _ => unreachable!(),
        }
    }

    fn read_texture_index(&mut self) -> Option<usize> {
        self.read_signed_index(self.header.texture_index_size)
    }

    fn read_material_index(&mut self) -> Option<usize> {
        self.read_signed_index(self.header.material_index_size)
    }

    fn read_bone_index(&mut self) -> Option<usize> {
        self.read_signed_index(self.header.bone_index_size)
    }

    fn read_morph_index(&mut self) -> Option<usize> {
        self.read_signed_index(self.header.morph_index_size)
    }

    fn read_rigid_index(&mut self) -> Option<usize> {
        self.read_signed_index(self.header.rigid_index_size)
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Error> {
        Ok(self.reader.seek(pos)?)
    }
}

struct DataIterator<'a, F, R>
where
    F: FnMut(&mut DataCursor<'a>) -> R,
{
    data: DataCursor<'a>,
    current: usize,
    len: usize,
    next: F,
}

impl<'a, F, R> DataIterator<'a, F, R>
where
    F: FnMut(&mut DataCursor<'a>) -> R,
{
    fn new(data: DataCursor<'a>, len: usize, next: F) -> Self {
        Self {
            data,
            current: 0,
            len,
            next,
        }
    }
}

impl<'a, F, R> Iterator for DataIterator<'a, F, R>
where
    F: FnMut(&mut DataCursor<'a>) -> R,
{
    type Item = R;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.len {
            return None;
        }
        let ret = (self.next)(&mut self.data);
        self.current += 1;
        Some(ret)
    }
}

impl<'a, F, R> ExactSizeIterator for DataIterator<'a, F, R>
where
    F: FnMut(&mut DataCursor<'a>) -> R,
{
    fn len(&self) -> usize {
        self.len
    }
}

pub struct Reader {
    data: Vec<u8>,
    header: Header,
    indices: Indices,
}

impl Reader {
    pub fn new<T: Read>(mut reader: T) -> Result<Self, Error> {
        let data = {
            let mut buffer = vec![];
            reader.read_to_end(&mut buffer)?;
            buffer
        };
        let mut reader = Cursor::new(&data);
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        if buffer != [b'P', b'M', b'X', b' '] {
            return Err(Error::invalid_header("magic number"));
        }
        reader.read_exact(&mut buffer)?;
        let version = f32::from_le_bytes(buffer);
        if version != 2.0 {
            return Err(Error::UnsupportedVersion);
        }
        let mut buffer = [0u8; 1];
        reader.read_exact(&mut buffer)?;
        let data_len = buffer[0];
        if data_len != 8 {
            return Err(Error::invalid_header("data length"));
        }
        let mut buffer = [0u8; 8];
        reader.read_exact(&mut buffer)?;
        let encoding = match buffer[0] {
            0 => Encoding::Utf16,
            1 => Encoding::Utf8,
            _ => return Err(Error::invalid_header("encoding")),
        };
        let extended_uv = buffer[1];
        if extended_uv > 4 {
            return Err(Error::invalid_header("extended uv"));
        }
        for index_size in &buffer[2..8] {
            match index_size {
                1 | 2 | 4 => {}
                _ => return Err(Error::invalid_header("index size")),
            }
        }
        let header = Header {
            encoding,
            extended_uv,
            vertex_index_size: buffer[2] as u64,
            texture_index_size: buffer[3] as u64,
            material_index_size: buffer[4] as u64,
            bone_index_size: buffer[5] as u64,
            morph_index_size: buffer[6] as u64,
            rigid_index_size: buffer[7] as u64,
        };
        let indices = Indices::new(&mut reader, &header)?;
        Ok(Self {
            data,
            header,
            indices,
        })
    }

    #[inline]
    pub fn name(&self) -> String {
        let mut data =
            DataCursor::with_position(&self.data, &self.header, SeekFrom::Start(self.indices.name));
        data.read_string()
    }

    #[inline]
    pub fn name_en(&self) -> String {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.name_en),
        );
        data.read_string()
    }

    #[inline]
    pub fn comment(&self) -> String {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.comment),
        );
        data.read_string()
    }

    #[inline]
    pub fn comment_en(&self) -> String {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.comment_en),
        );
        data.read_string()
    }

    #[inline]
    pub fn vertices(&self) -> impl ExactSizeIterator<Item = Vertex> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.vertices),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| {
            let position = data.read_vec3();
            let normal = data.read_vec3();
            let uv = data.read_vec2();
            let extended_uv = (0..data.header.extended_uv)
                .map(|_| data.read_vec4())
                .collect::<Vec<_>>();
            let weight = match data.read_u8() {
                0 => Weight::Bdef1(Bdef1 {
                    bone: data.read_bone_index(),
                }),
                1 => Weight::Bdef2(Bdef2 {
                    bones: [data.read_bone_index(), data.read_bone_index()],
                    weight: data.read_f32(),
                }),
                2 => Weight::Bdef4(Bdef4 {
                    bones: [
                        data.read_bone_index(),
                        data.read_bone_index(),
                        data.read_bone_index(),
                        data.read_bone_index(),
                    ],
                    weights: [
                        data.read_f32(),
                        data.read_f32(),
                        data.read_f32(),
                        data.read_f32(),
                    ],
                }),
                3 => Weight::Sdef(Sdef {
                    bones: [data.read_bone_index(), data.read_bone_index()],
                    weight: data.read_f32(),
                    c: data.read_vec3(),
                    r0: data.read_vec3(),
                    r1: data.read_vec3(),
                }),
                _ => unreachable!(),
            };
            let edge_ratio = data.read_f32();
            Vertex {
                position,
                normal,
                uv,
                extended_uv,
                weight,
                edge_ratio,
            }
        };
        DataIterator::new(data, len, f)
    }

    #[inline]
    pub fn faces(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.faces),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| data.read_vertex_index();
        DataIterator::new(data, len, f)
    }

    #[inline]
    pub fn textures(&self) -> impl ExactSizeIterator<Item = PathBuf> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.textures),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| data.read_string().into();
        DataIterator::new(data, len, f)
    }

    #[inline]
    pub fn materials(&self) -> impl ExactSizeIterator<Item = Material> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.materials),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| {
            let name = data.read_string();
            let name_en = data.read_string();
            let diffuse = data.read_vec4();
            let specular = data.read_vec3();
            let specular_power = data.read_f32();
            let ambient = data.read_vec3();
            let flags = data.read_u8();
            let both = flags & 0x01 != 0;
            let ground_shadow = flags & 0x02 != 0;
            let self_shadow_map = flags & 0x04 != 0;
            let self_shadow = flags & 0x08 != 0;
            let edge = flags & 0x10 != 0;
            let edge_color = data.read_vec4();
            let edge_size = data.read_f32();
            let texture = data.read_texture_index();
            let sphere = data.read_texture_index();
            let sphere_mode = match data.read_u8() {
                0 => SphereMode::None,
                1 => SphereMode::Add,
                2 => SphereMode::Mul,
                3 => SphereMode::SubTexture,
                _ => unreachable!(),
            };
            let toon = match data.read_u8() {
                0 => Toon::Texture(data.read_texture_index()),
                1 => Toon::Shared(data.read_u8()),
                _ => unreachable!(),
            };
            let memo = data.read_string();
            let index_count = data.read_u32();
            Material {
                name,
                name_en,
                diffuse,
                specular,
                specular_power,
                ambient,
                both,
                ground_shadow,
                self_shadow_map,
                self_shadow,
                edge,
                edge_color,
                edge_size,
                texture,
                sphere,
                sphere_mode,
                toon,
                memo,
                index_count,
            }
        };
        DataIterator::new(data, len, f)
    }

    #[inline]
    pub fn bones(&self) -> impl ExactSizeIterator<Item = Bone> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.bones),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| {
            let name = data.read_string();
            let name_en = data.read_string();
            let position = data.read_vec3();
            let parent = data.read_bone_index();
            let deform_hierarchy = data.read_i32();
            let flags = data.read_u16();
            let connected_to = flags & 0x0001;
            let rotatable = flags & 0x0002 != 0;
            let translatable = flags & 0x0004 != 0;
            let visibility = flags & 0x0008 != 0;
            let operable = flags & 0x0010 != 0;
            let ik = flags & 0x0020 != 0;
            let addition_local = flags & 0x0040 != 0;
            let addition_rotation = flags & 0x0080 != 0;
            let addition_translation = flags & 0x0100 != 0;
            let fixed_pole = flags & 0x0400 != 0;
            let local_pole = flags & 0x0800 != 0;
            let after_physics = flags & 0x1000 != 0;
            let external_parent = flags & 0x2000 != 0;
            let connected_to = match connected_to {
                0 => ConnectTo::Offset(data.read_vec3()),
                1 => ConnectTo::Bone(data.read_bone_index()),
                _ => unreachable!(),
            };
            let addition = (addition_rotation || addition_translation).then(|| Addition {
                rotation: addition_rotation,
                translation: addition_translation,
                local: addition_local,
                bone: data.read_bone_index(),
                ratio: data.read_f32(),
            });
            let fixed_pole = fixed_pole.then(|| data.read_vec3());
            let local_pole = local_pole.then(|| LocalPole {
                x: data.read_vec3(),
                z: data.read_vec3(),
            });
            let external_parent = external_parent.then(|| data.read_i32() as usize);
            let ik = ik.then(|| {
                let target_bone = data.read_bone_index();
                let loop_count = data.read_u32();
                let angle = data.read_f32();
                let link_len = data.read_u32();
                let links = (0..link_len)
                    .map(|_| {
                        let bone = data.read_bone_index();
                        let limit = (data.read_u8() == 1).then(|| AngleLimit {
                            lower: data.read_vec3(),
                            upper: data.read_vec3(),
                        });
                        IkLink { bone, limit }
                    })
                    .collect::<Vec<_>>();
                Ik {
                    target_bone,
                    loop_count,
                    angle,
                    links,
                }
            });
            Bone {
                name,
                name_en,
                position,
                parent,
                deform_hierarchy,
                connected_to,
                rotatable,
                translatable,
                visibility,
                operable,
                after_physics,
                ik,
                addition,
                fixed_pole,
                local_pole,
                external_parent,
            }
        };
        DataIterator::new(data, len, f)
    }

    #[inline]
    pub fn morphs(&self) -> impl ExactSizeIterator<Item = Morph> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.morphs),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| {
            let name = data.read_string();
            let name_en = data.read_string();
            let panel = match data.read_u8() {
                0 => Panel::Reserved,
                1 => Panel::Eyebrow,
                2 => Panel::Eye,
                3 => Panel::Mouth,
                4 => Panel::Other,
                _ => unreachable!(),
            };
            let kind = data.read_u8();
            let len = data.read_u32();
            let kind = match kind {
                0 => morph::Kind::Group(
                    (0..len)
                        .map(|_| morph::Group {
                            morph: data.read_morph_index(),
                            ratio: data.read_f32(),
                        })
                        .collect::<Vec<_>>(),
                ),
                1 => morph::Kind::Vertex(
                    (0..len)
                        .map(|_| morph::Vertex {
                            vertex: data.read_vertex_index(),
                            offset: data.read_vec3(),
                        })
                        .collect::<Vec<_>>(),
                ),
                2 => morph::Kind::Bone(
                    (0..len)
                        .map(|_| morph::Bone {
                            bone: data.read_bone_index(),
                            offset: data.read_vec3(),
                            rotation: data.read_vec4(),
                        })
                        .collect::<Vec<_>>(),
                ),
                3 => morph::Kind::Uv(
                    (0..len)
                        .map(|_| morph::Uv {
                            vertex: data.read_vertex_index(),
                            offset: data.read_vec4(),
                        })
                        .collect::<Vec<_>>(),
                ),
                v @ (4 | 5 | 6 | 7) => morph::Kind::ExtendedUv(
                    v as usize - 4,
                    (0..len)
                        .map(|_| morph::Uv {
                            vertex: data.read_vertex_index(),
                            offset: data.read_vec4(),
                        })
                        .collect::<Vec<_>>(),
                ),
                8 => morph::Kind::Material(
                    (0..len)
                        .map(|_| morph::Material {
                            material: data.read_material_index(),
                            op: match data.read_u8() {
                                0 => morph::MaterialOp::Mul,
                                1 => morph::MaterialOp::Add,
                                _ => unreachable!(),
                            },
                            diffuse: data.read_vec4(),
                            specular: data.read_vec3(),
                            specular_power: data.read_f32(),
                            ambient: data.read_vec3(),
                            edge_color: data.read_vec4(),
                            edge_size: data.read_f32(),
                            texture: data.read_vec4(),
                            sphere: data.read_vec4(),
                            toon: data.read_vec4(),
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => unreachable!(),
            };
            Morph {
                name,
                name_en,
                panel,
                kind,
            }
        };
        DataIterator::new(data, len, f)
    }

    #[inline]
    pub fn display_groups(&self) -> impl ExactSizeIterator<Item = DisplayGroup> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.display_groups),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| {
            let name = data.read_string();
            let name_en = data.read_string();
            let special = data.read_u8() != 0;
            let len = data.read_u32();
            let elements = (0..len)
                .map(|_| match data.read_u8() {
                    0 => DisplayElement::Bone(data.read_bone_index()),
                    1 => DisplayElement::Morph(data.read_morph_index()),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>();
            DisplayGroup {
                name,
                name_en,
                special,
                elements,
            }
        };
        DataIterator::new(data, len, f)
    }

    #[inline]
    pub fn rigids(&self) -> impl ExactSizeIterator<Item = Rigid> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.rigids),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| {
            let name = data.read_string();
            let name_en = data.read_string();
            let bone = data.read_bone_index();
            let group = data.read_u8();
            let non_collision_groups = data.read_u16();
            let shape = match data.read_u8() {
                0 => rigid::Shape::Sphere,
                1 => rigid::Shape::Box,
                2 => rigid::Shape::Capsule,
                _ => unreachable!(),
            };
            let size = data.read_vec3();
            let position = data.read_vec3();
            let rotation = data.read_vec3();
            let mass = data.read_f32();
            let dump_translation = data.read_f32();
            let dump_rotation = data.read_f32();
            let repulsive = data.read_f32();
            let friction = data.read_f32();
            let method = match data.read_u8() {
                0 => rigid::Method::Static,
                1 => rigid::Method::Dynamic,
                2 => rigid::Method::DynamicWithBone,
                _ => unreachable!(),
            };
            Rigid {
                name,
                name_en,
                bone,
                group,
                non_collision_groups,
                shape,
                size,
                position,
                rotation,
                mass,
                dump_translation,
                dump_rotation,
                repulsive,
                friction,
                method,
            }
        };
        DataIterator::new(data, len, f)
    }

    #[inline]
    pub fn joints(&self) -> impl ExactSizeIterator<Item = Joint> + '_ {
        let mut data = DataCursor::with_position(
            &self.data,
            &self.header,
            SeekFrom::Start(self.indices.joints),
        );
        let len = data.read_u32() as usize;
        let f = |data: &mut DataCursor| {
            let name = data.read_string();
            let name_en = data.read_string();
            let ty = data.read_u8();
            assert!(ty == 0);
            let rigids = [data.read_rigid_index(), data.read_rigid_index()];
            let position = data.read_vec3();
            let rotation = data.read_vec3();
            let limit_translation = AngleLimit {
                lower: data.read_vec3(),
                upper: data.read_vec3(),
            };
            let limit_rotation = AngleLimit {
                lower: data.read_vec3(),
                upper: data.read_vec3(),
            };
            let spring_translation = data.read_vec3();
            let spring_rotation = data.read_vec3();
            Joint {
                name,
                name_en,
                rigids,
                position,
                rotation,
                limit_translation,
                limit_rotation,
                spring_translation,
                spring_rotation,
            }
        };
        DataIterator::new(data, len, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_reader() -> Reader {
        Reader::new(Cursor::new(include_bytes!(
            "../assets/Alicia/Alicia_solid.pmx"
        )))
        .unwrap()
    }

    #[test]
    fn name() {
        let reader = new_reader();
        assert!(reader.name() == "アリシア・ソリッド");
    }

    #[test]
    fn len() {
        let reader = new_reader();
        assert!(reader.vertices().len() == 22311);
        assert!(reader.faces().len() == 95598);
        assert!(reader.materials().len() == 22);
        assert!(reader.bones().len() == 150);
        assert!(reader.rigids().len() == 79);
        assert!(reader.joints().len() == 53);
    }

    #[test]
    fn last_material() {
        let reader = new_reader();
        let textures = reader.textures().collect::<Vec<_>>();
        let material = reader.materials().last().unwrap();
        assert!(material.name == "maegami");
        assert!(textures[material.texture.unwrap()].to_string_lossy() == "Alicia_hair.tga");
        assert!(material.both == true);
        assert!(material.ground_shadow == true);
        assert!(material.self_shadow_map == true);
        assert!(material.self_shadow == true);
        assert!(material.edge == true);
        assert!(material.index_count == 296 * 3);
    }

    #[test]
    fn last_joint() {
        let reader = new_reader();
        let joint = reader.joints().last().unwrap();
        assert!(joint.name == "リボン右");
        let lower_z = joint.limit_rotation.lower[2];
        let upper_z = joint.limit_rotation.upper[2];
        let al = -5.0f32.to_radians();
        let l = (lower_z - al).abs();
        let au = 20.0f32.to_radians();
        let u = (upper_z - au).abs();
        assert!(l / lower_z.abs() <= f32::EPSILON || l / al.abs() <= f32::EPSILON);
        assert!(u / upper_z.abs() <= f32::EPSILON || u / au.abs() <= f32::EPSILON);
    }
}
