use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const MAX_REASONABLE_SIZE: u64 = 1 << 31;

pub struct PEMemoryMapper {
    base_address: u64,
    memory_size: u64,
    memory: Vec<u8>,
}

impl PEMemoryMapper {
    pub fn new(path: &str) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let file_size = file.metadata().ok()?.len();

        let mut dos_header = [0u8; 64];
        file.read_exact(&mut dos_header).ok()?;

        let e_magic = u16::from_le_bytes([dos_header[0], dos_header[1]]);
        if e_magic != 0x5A4D {
            return None;
        }

        let e_lfanew = u32::from_le_bytes([
            dos_header[0x3C],
            dos_header[0x3D],
            dos_header[0x3E],
            dos_header[0x3F],
        ]) as u64;

        file.seek(SeekFrom::Start(e_lfanew)).ok()?;
        let mut signature = [0u8; 4];
        file.read_exact(&mut signature).ok()?;
        if u32::from_le_bytes(signature) != 0x00004550 {
            return None;
        }

        let mut file_header = [0u8; 20];
        file.read_exact(&mut file_header).ok()?;

        let number_of_sections = u16::from_le_bytes([file_header[2], file_header[3]]) as usize;
        let size_of_optional_header = u16::from_le_bytes([file_header[16], file_header[17]]) as u64;

        let optional_header_offset = e_lfanew + 4 + 20;
        file.seek(SeekFrom::Start(optional_header_offset)).ok()?;
        let mut magic = [0u8; 2];
        file.read_exact(&mut magic).ok()?;
        let magic_val = u16::from_le_bytes(magic);

        let (base_address, memory_size) = if magic_val == 0x10B {
            file.seek(SeekFrom::Start(optional_header_offset + 28))
                .ok()?;
            let mut buf = [0u8; 4];
            file.read_exact(&mut buf).ok()?;
            let image_base = u32::from_le_bytes(buf) as u64;
            file.seek(SeekFrom::Start(optional_header_offset + 56))
                .ok()?;
            file.read_exact(&mut buf).ok()?;
            let size_of_image = u32::from_le_bytes(buf) as u64;
            (image_base, size_of_image)
        } else if magic_val == 0x20B {
            file.seek(SeekFrom::Start(optional_header_offset + 24))
                .ok()?;
            let mut buf = [0u8; 8];
            file.read_exact(&mut buf).ok()?;
            let image_base = u64::from_le_bytes(buf);
            file.seek(SeekFrom::Start(optional_header_offset + 56))
                .ok()?;
            let mut buf4 = [0u8; 4];
            file.read_exact(&mut buf4).ok()?;
            let size_of_image = u32::from_le_bytes(buf4) as u64;
            (image_base, size_of_image)
        } else {
            return None;
        };

        if memory_size == 0 || memory_size > MAX_REASONABLE_SIZE {
            return None;
        }

        let mut memory = vec![0u8; memory_size as usize];

        let size_of_headers_offset = optional_header_offset + 60;
        file.seek(SeekFrom::Start(size_of_headers_offset)).ok()?;
        let mut hdr_buf = [0u8; 4];
        file.read_exact(&mut hdr_buf).ok()?;
        let size_of_headers = u32::from_le_bytes(hdr_buf) as usize;
        let headers_copy = size_of_headers.min(memory.len()).min(file_size as usize);
        file.seek(SeekFrom::Start(0)).ok()?;
        file.read_exact(&mut memory[..headers_copy]).ok()?;

        let section_headers_offset = e_lfanew + 4 + 20 + size_of_optional_header;

        for i in 0..number_of_sections {
            let section_offset = section_headers_offset + (i as u64 * 40);
            file.seek(SeekFrom::Start(section_offset)).ok()?;
            let mut section = [0u8; 40];
            file.read_exact(&mut section).ok()?;

            let virtual_size =
                u32::from_le_bytes([section[8], section[9], section[10], section[11]]) as u64;
            let virtual_address =
                u32::from_le_bytes([section[12], section[13], section[14], section[15]]) as u64;
            let size_of_raw_data =
                u32::from_le_bytes([section[16], section[17], section[18], section[19]]) as u64;
            let pointer_to_raw_data =
                u32::from_le_bytes([section[20], section[21], section[22], section[23]]) as u64;

            let vs = if virtual_size == 0 {
                size_of_raw_data
            } else {
                virtual_size
            };

            if virtual_address >= memory_size
                || vs > memory_size
                || virtual_address + vs > memory_size
            {
                return None;
            }

            let copy_size = size_of_raw_data.min(vs) as usize;
            if pointer_to_raw_data + copy_size as u64 > file_size {
                return None;
            }

            file.seek(SeekFrom::Start(pointer_to_raw_data)).ok()?;
            file.read_exact(
                &mut memory[virtual_address as usize..virtual_address as usize + copy_size],
            )
            .ok()?;
        }

        Some(Self {
            base_address,
            memory_size,
            memory,
        })
    }

    pub fn read_from_va(&self, va: u64, size: usize) -> Option<Vec<u8>> {
        let offset = va.checked_sub(self.base_address)? as usize;
        if offset + size > self.memory.len() {
            return None;
        }
        Some(self.memory[offset..offset + size].to_vec())
    }

    pub fn write_to_va(&mut self, va: u64, data: &[u8]) -> bool {
        let offset = match va.checked_sub(self.base_address) {
            Some(o) => o as usize,
            None => return false,
        };
        if offset + data.len() > self.memory.len() {
            return false;
        }
        self.memory[offset..offset + data.len()].copy_from_slice(data);
        true
    }

    pub fn is_va_mapped(&self, va: u64) -> bool {
        match va.checked_sub(self.base_address) {
            Some(offset) => (offset as usize) < self.memory.len(),
            None => false,
        }
    }

    pub fn get_data_pointer(&self, va: u64) -> Option<*const u8> {
        let offset = va.checked_sub(self.base_address)? as usize;
        if offset >= self.memory.len() {
            return None;
        }
        Some(unsafe { self.memory.as_ptr().add(offset) })
    }

    pub fn get_memory(&self) -> &[u8] {
        &self.memory
    }

    pub fn get_memory_mut(&mut self) -> &mut [u8] {
        &mut self.memory
    }

    pub fn base_address(&self) -> u64 {
        self.base_address
    }

    pub fn memory_size(&self) -> u64 {
        self.memory_size
    }

    pub fn sigscan(&self, pattern: &str) -> Option<usize> {
        let signature = hex_to_bytes(pattern);
        if signature.is_empty() {
            return None;
        }

        let first = signature[0];
        let base = self.memory.as_ptr();
        let end = self.memory.len().saturating_sub(signature.len());

        for i in 0..end {
            unsafe {
                if *base.add(i) != first {
                    continue;
                }
                let mut matched = true;
                for (j, &byte) in signature.iter().enumerate() {
                    if byte == b'?' {
                        continue;
                    }
                    if *base.add(i + j) != byte {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    return Some(base.add(i) as usize);
                }
            }
        }

        None
    }

    pub fn read<T: Copy>(&self, va: u64) -> Option<T> {
        let data = self.read_from_va(va, std::mem::size_of::<T>())?;
        Some(unsafe { std::ptr::read(data.as_ptr() as *const T) })
    }
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let hex: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    let chars: Vec<char> = hex.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '?' {
            bytes.push(b'?');
            i += 1;
            if i < chars.len() && chars[i] == '?' {
                i += 1;
            }
        } else {
            let byte_str: String = chars[i..std::cmp::min(i + 2, chars.len())].iter().collect();
            if let Ok(byte) = u8::from_str_radix(&byte_str, 16) {
                bytes.push(byte);
            }
            i += 2;
        }
    }

    bytes
}
