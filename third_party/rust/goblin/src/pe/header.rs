use crate::error;
use crate::pe::{optional_header, section_table, symbol};
use crate::strtab;
use alloc::vec::Vec;
use log::debug;
use scroll::{ctx, IOread, IOwrite, Pread, Pwrite, SizeWith};

/// DOS header present in all PE binaries
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone, Default, Pwrite)]
pub struct DosHeader {
    /// Magic number: 5a4d
    pub signature: u16,
    /// e_cblp
    pub bytes_on_last_page: u16,
    /// e_cp
    pub pages_in_file: u16,
    /// e_crlc
    pub relocations: u16,
    /// e_cparhdr
    pub size_of_header_in_paragraphs: u16,
    /// e_minalloc
    pub minimum_extra_paragraphs_needed: u16,
    /// e_maxalloc
    pub maximum_extra_paragraphs_needed: u16,
    /// e_ss
    pub initial_relative_ss: u16,
    /// e_sp
    pub initial_sp: u16,
    /// e_csum
    pub checksum: u16,
    /// e_ip
    pub initial_ip: u16,
    /// e_cs
    pub initial_relative_cs: u16,
    /// e_lfarlc
    pub file_address_of_relocation_table: u16,
    /// e_ovno
    pub overlay_number: u16,
    /// e_res[4]
    pub reserved: [u16; 4],
    /// e_oemid
    pub oem_id: u16,
    /// e_oeminfo
    pub oem_info: u16,
    /// e_res2[10]
    pub reserved2: [u16; 10],
    /// e_lfanew: pointer to PE header, always at offset 0x3c
    pub pe_pointer: u32,
}

pub const DOS_MAGIC: u16 = 0x5a4d;
pub const PE_POINTER_OFFSET: u32 = 0x3c;
pub const DOS_STUB_OFFSET: u32 = PE_POINTER_OFFSET + (core::mem::size_of::<u32>() as u32);

impl DosHeader {
    pub fn parse(bytes: &[u8]) -> error::Result<Self> {
        let mut offset = 0;
        let signature = bytes.gread_with(&mut offset, scroll::LE).map_err(|_| {
            error::Error::Malformed(format!("cannot parse DOS signature (offset {:#x})", 0))
        })?;
        if signature != DOS_MAGIC {
            return Err(error::Error::Malformed(format!(
                "DOS header is malformed (signature {:#x})",
                signature
            )));
        }

        let bytes_on_last_page = bytes.gread_with(&mut offset, scroll::LE)?;
        let pages_in_file = bytes.gread_with(&mut offset, scroll::LE)?;
        let relocations = bytes.gread_with(&mut offset, scroll::LE)?;
        let size_of_header_in_paragraphs = bytes.gread_with(&mut offset, scroll::LE)?;
        let minimum_extra_paragraphs_needed = bytes.gread_with(&mut offset, scroll::LE)?;
        let maximum_extra_paragraphs_needed = bytes.gread_with(&mut offset, scroll::LE)?;
        let initial_relative_ss = bytes.gread_with(&mut offset, scroll::LE)?;
        let initial_sp = bytes.gread_with(&mut offset, scroll::LE)?;
        let checksum = bytes.gread_with(&mut offset, scroll::LE)?;
        let initial_ip = bytes.gread_with(&mut offset, scroll::LE)?;
        let initial_relative_cs = bytes.gread_with(&mut offset, scroll::LE)?;
        let file_address_of_relocation_table = bytes.gread_with(&mut offset, scroll::LE)?;
        let overlay_number = bytes.gread_with(&mut offset, scroll::LE)?;
        let reserved = [0x0; 4];
        offset += core::mem::size_of_val(&reserved);
        let oem_id = bytes.gread_with(&mut offset, scroll::LE)?;
        let oem_info = bytes.gread_with(&mut offset, scroll::LE)?;
        let reserved2 = [0x0; 10];
        offset += core::mem::size_of_val(&reserved2);

        debug_assert!(
            offset == PE_POINTER_OFFSET as usize,
            "expected offset ({:#x}) after reading DOS header to be at 0x3C",
            offset
        );

        let pe_pointer = bytes
            .pread_with(PE_POINTER_OFFSET as usize, scroll::LE)
            .map_err(|_| {
                error::Error::Malformed(format!(
                    "cannot parse PE header pointer (offset {:#x})",
                    PE_POINTER_OFFSET
                ))
            })?;

        let pe_signature: u32 =
            bytes
                .pread_with(pe_pointer as usize, scroll::LE)
                .map_err(|_| {
                    error::Error::Malformed(format!(
                        "cannot parse PE header signature (offset {:#x})",
                        pe_pointer
                    ))
                })?;
        if pe_signature != PE_MAGIC {
            return Err(error::Error::Malformed(format!(
                "PE header is malformed (signature {:#x})",
                pe_signature
            )));
        }

        Ok(DosHeader {
            signature,
            bytes_on_last_page,
            pages_in_file,
            relocations,
            size_of_header_in_paragraphs,
            minimum_extra_paragraphs_needed,
            maximum_extra_paragraphs_needed,
            initial_relative_ss,
            initial_sp,
            checksum,
            initial_ip,
            initial_relative_cs,
            file_address_of_relocation_table,
            overlay_number,
            reserved,
            oem_id,
            oem_info,
            reserved2,
            pe_pointer,
        })
    }
}

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone, Pread, Pwrite)]
/// The DOS stub program which should be executed in DOS mode
pub struct DosStub(pub [u8; 0x40]);
impl Default for DosStub {
    fn default() -> Self {
        // "This program cannot be run in DOS mode" error program
        Self([
            0x0E, 0x1F, 0xBA, 0x0E, 0x00, 0xB4, 0x09, 0xCD, 0x21, 0xB8, 0x01, 0x4C, 0xCD, 0x21,
            0x54, 0x68, 0x69, 0x73, 0x20, 0x70, 0x72, 0x6F, 0x67, 0x72, 0x61, 0x6D, 0x20, 0x63,
            0x61, 0x6E, 0x6E, 0x6F, 0x74, 0x20, 0x62, 0x65, 0x20, 0x72, 0x75, 0x6E, 0x20, 0x69,
            0x6E, 0x20, 0x44, 0x4F, 0x53, 0x20, 0x6D, 0x6F, 0x64, 0x65, 0x2E, 0x0D, 0x0D, 0x0A,
            0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])
    }
}

/// COFF Header
#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone, Default, Pread, Pwrite, IOread, IOwrite, SizeWith)]
pub struct CoffHeader {
    /// The machine type
    pub machine: u16,
    pub number_of_sections: u16,
    pub time_date_stamp: u32,
    pub pointer_to_symbol_table: u32,
    pub number_of_symbol_table: u32,
    pub size_of_optional_header: u16,
    pub characteristics: u16,
}

pub const SIZEOF_COFF_HEADER: usize = 20;
/// PE\0\0, little endian
pub const PE_MAGIC: u32 = 0x0000_4550;
pub const SIZEOF_PE_MAGIC: usize = 4;
/// The contents of this field are assumed to be applicable to any machine type
pub const COFF_MACHINE_UNKNOWN: u16 = 0x0;
/// Matsushita AM33
pub const COFF_MACHINE_AM33: u16 = 0x1d3;
/// x64
pub const COFF_MACHINE_X86_64: u16 = 0x8664;
/// ARM little endian
pub const COFF_MACHINE_ARM: u16 = 0x1c0;
/// ARM64 little endian
pub const COFF_MACHINE_ARM64: u16 = 0xaa64;
/// ARM Thumb-2 little endian
pub const COFF_MACHINE_ARMNT: u16 = 0x1c4;
/// EFI byte code
pub const COFF_MACHINE_EBC: u16 = 0xebc;
/// Intel 386 or later processors and compatible processors
pub const COFF_MACHINE_X86: u16 = 0x14c;
/// Intel Itanium processor family
pub const COFF_MACHINE_IA64: u16 = 0x200;
/// Mitsubishi M32R little endian
pub const COFF_MACHINE_M32R: u16 = 0x9041;
/// MIPS16
pub const COFF_MACHINE_MIPS16: u16 = 0x266;
/// MIPS with FPU
pub const COFF_MACHINE_MIPSFPU: u16 = 0x366;
/// MIPS16 with FPU
pub const COFF_MACHINE_MIPSFPU16: u16 = 0x466;
/// Power PC little endian
pub const COFF_MACHINE_POWERPC: u16 = 0x1f0;
/// Power PC with floating point support
pub const COFF_MACHINE_POWERPCFP: u16 = 0x1f1;
/// MIPS little endian
pub const COFF_MACHINE_R4000: u16 = 0x166;
/// RISC-V 32-bit address space
pub const COFF_MACHINE_RISCV32: u16 = 0x5032;
/// RISC-V 64-bit address space
pub const COFF_MACHINE_RISCV64: u16 = 0x5064;
/// RISC-V 128-bit address space
pub const COFF_MACHINE_RISCV128: u16 = 0x5128;
/// Hitachi SH3
pub const COFF_MACHINE_SH3: u16 = 0x1a2;
/// Hitachi SH3 DSP
pub const COFF_MACHINE_SH3DSP: u16 = 0x1a3;
/// Hitachi SH4
pub const COFF_MACHINE_SH4: u16 = 0x1a6;
/// Hitachi SH5
pub const COFF_MACHINE_SH5: u16 = 0x1a8;
/// Thumb
pub const COFF_MACHINE_THUMB: u16 = 0x1c2;
/// MIPS little-endian WCE v2
pub const COFF_MACHINE_WCEMIPSV2: u16 = 0x169;

impl CoffHeader {
    pub fn parse(bytes: &[u8], offset: &mut usize) -> error::Result<Self> {
        Ok(bytes.gread_with(offset, scroll::LE)?)
    }

    /// Parse the COFF section headers.
    ///
    /// For COFF, these immediately follow the COFF header. For PE, these immediately follow the
    /// optional header.
    pub fn sections(
        &self,
        bytes: &[u8],
        offset: &mut usize,
    ) -> error::Result<Vec<section_table::SectionTable>> {
        let nsections = self.number_of_sections as usize;

        // a section table is at least 40 bytes
        if nsections > bytes.len() / 40 {
            return Err(error::Error::BufferTooShort(nsections, "sections"));
        }

        let mut sections = Vec::with_capacity(nsections);
        // Note that if we are handling a BigCoff, the size of the symbol will be different!
        let string_table_offset = self.pointer_to_symbol_table as usize
            + symbol::SymbolTable::size(self.number_of_symbol_table as usize);
        for i in 0..nsections {
            let section =
                section_table::SectionTable::parse(bytes, offset, string_table_offset as usize)?;
            debug!("({}) {:#?}", i, section);
            sections.push(section);
        }
        Ok(sections)
    }

    /// Return the COFF symbol table.
    pub fn symbols<'a>(&self, bytes: &'a [u8]) -> error::Result<Option<symbol::SymbolTable<'a>>> {
        let offset = self.pointer_to_symbol_table as usize;
        let number = self.number_of_symbol_table as usize;
        if offset == 0 {
            Ok(None)
        } else {
            symbol::SymbolTable::parse(bytes, offset, number).map(Some)
        }
    }

    /// Return the COFF string table.
    pub fn strings<'a>(&self, bytes: &'a [u8]) -> error::Result<Option<strtab::Strtab<'a>>> {
        // > The file offset of the COFF symbol table, or zero if no COFF symbol table is present.
        // > This value should be zero for an image because COFF debugging information is deprecated.
        if self.pointer_to_symbol_table == 0 {
            return Ok(None);
        }

        let mut offset = self.pointer_to_symbol_table as usize
            + symbol::SymbolTable::size(self.number_of_symbol_table as usize);

        let length_field_size = core::mem::size_of::<u32>();
        let length = bytes.pread_with::<u32>(offset, scroll::LE)? as usize - length_field_size;

        // The offset needs to be advanced in order to read the strings.
        offset += length_field_size;

        Ok(Some(strtab::Strtab::parse(bytes, offset, length, 0)?))
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Default)]
pub struct Header {
    pub dos_header: DosHeader,
    /// DOS program for legacy loaders
    pub dos_stub: DosStub,
    /// PE Magic: PE\0\0, little endian
    pub signature: u32,
    pub coff_header: CoffHeader,
    pub optional_header: Option<optional_header::OptionalHeader>,
}

impl Header {
    pub fn parse(bytes: &[u8]) -> error::Result<Self> {
        let dos_header = DosHeader::parse(&bytes)?;
        let dos_stub = bytes.pread(DOS_STUB_OFFSET as usize).map_err(|_| {
            error::Error::Malformed(format!(
                "cannot parse DOS stub (offset {:#x})",
                DOS_STUB_OFFSET
            ))
        })?;
        let mut offset = dos_header.pe_pointer as usize;
        let signature = bytes.gread_with(&mut offset, scroll::LE).map_err(|_| {
            error::Error::Malformed(format!("cannot parse PE signature (offset {:#x})", offset))
        })?;
        let coff_header = CoffHeader::parse(&bytes, &mut offset)?;
        let optional_header = if coff_header.size_of_optional_header > 0 {
            Some(bytes.pread::<optional_header::OptionalHeader>(offset)?)
        } else {
            None
        };
        Ok(Header {
            dos_header,
            dos_stub,
            signature,
            coff_header,
            optional_header,
        })
    }
}

impl ctx::TryIntoCtx<scroll::Endian> for Header {
    type Error = error::Error;

    fn try_into_ctx(self, bytes: &mut [u8], ctx: scroll::Endian) -> Result<usize, Self::Error> {
        let offset = &mut 0;
        bytes.gwrite_with(self.dos_header, offset, ctx)?;
        bytes.gwrite_with(self.dos_stub, offset, ctx)?;
        bytes.gwrite_with(self.signature, offset, scroll::LE)?;
        bytes.gwrite_with(self.coff_header, offset, ctx)?;
        if let Some(opt_header) = self.optional_header {
            bytes.gwrite_with(opt_header, offset, ctx)?;
        }
        Ok(*offset)
    }
}

/// Convert machine to str representation
pub fn machine_to_str(machine: u16) -> &'static str {
    match machine {
        COFF_MACHINE_UNKNOWN => "UNKNOWN",
        COFF_MACHINE_AM33 => "AM33",
        COFF_MACHINE_X86_64 => "X86_64",
        COFF_MACHINE_ARM => "ARM",
        COFF_MACHINE_ARM64 => "ARM64",
        COFF_MACHINE_ARMNT => "ARM_NT",
        COFF_MACHINE_EBC => "EBC",
        COFF_MACHINE_X86 => "X86",
        COFF_MACHINE_IA64 => "IA64",
        COFF_MACHINE_M32R => "M32R",
        COFF_MACHINE_MIPS16 => "MIPS_16",
        COFF_MACHINE_MIPSFPU => "MIPS_FPU",
        COFF_MACHINE_MIPSFPU16 => "MIPS_FPU_16",
        COFF_MACHINE_POWERPC => "POWERPC",
        COFF_MACHINE_POWERPCFP => "POWERCFP",
        COFF_MACHINE_R4000 => "R4000",
        COFF_MACHINE_RISCV32 => "RISC-V_32",
        COFF_MACHINE_RISCV64 => "RISC-V_64",
        COFF_MACHINE_RISCV128 => "RISC-V_128",
        COFF_MACHINE_SH3 => "SH3",
        COFF_MACHINE_SH3DSP => "SH3DSP",
        COFF_MACHINE_SH4 => "SH4",
        COFF_MACHINE_SH5 => "SH5",
        COFF_MACHINE_THUMB => "THUMB",
        COFF_MACHINE_WCEMIPSV2 => "WCE_MIPS_V2",
        _ => "COFF_UNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::{machine_to_str, Header, COFF_MACHINE_X86, DOS_MAGIC, PE_MAGIC};

    const CRSS_HEADER: [u8; 688] = [
        0x4d, 0x5a, 0x90, 0x00, 0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00,
        0x00, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xd0, 0x00, 0x00, 0x00, 0x0e, 0x1f, 0xba, 0x0e, 0x00, 0xb4, 0x09, 0xcd, 0x21, 0xb8, 0x01,
        0x4c, 0xcd, 0x21, 0x54, 0x68, 0x69, 0x73, 0x20, 0x70, 0x72, 0x6f, 0x67, 0x72, 0x61, 0x6d,
        0x20, 0x63, 0x61, 0x6e, 0x6e, 0x6f, 0x74, 0x20, 0x62, 0x65, 0x20, 0x72, 0x75, 0x6e, 0x20,
        0x69, 0x6e, 0x20, 0x44, 0x4f, 0x53, 0x20, 0x6d, 0x6f, 0x64, 0x65, 0x2e, 0x0d, 0x0d, 0x0a,
        0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xaa, 0x4a, 0xc3, 0xeb, 0xee, 0x2b, 0xad,
        0xb8, 0xee, 0x2b, 0xad, 0xb8, 0xee, 0x2b, 0xad, 0xb8, 0xee, 0x2b, 0xac, 0xb8, 0xfe, 0x2b,
        0xad, 0xb8, 0x33, 0xd4, 0x66, 0xb8, 0xeb, 0x2b, 0xad, 0xb8, 0x33, 0xd4, 0x63, 0xb8, 0xea,
        0x2b, 0xad, 0xb8, 0x33, 0xd4, 0x7a, 0xb8, 0xed, 0x2b, 0xad, 0xb8, 0x33, 0xd4, 0x64, 0xb8,
        0xef, 0x2b, 0xad, 0xb8, 0x33, 0xd4, 0x61, 0xb8, 0xef, 0x2b, 0xad, 0xb8, 0x52, 0x69, 0x63,
        0x68, 0xee, 0x2b, 0xad, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x50, 0x45,
        0x00, 0x00, 0x4c, 0x01, 0x05, 0x00, 0xd9, 0x8f, 0x15, 0x52, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0xe0, 0x00, 0x02, 0x01, 0x0b, 0x01, 0x0b, 0x00, 0x00, 0x08, 0x00, 0x00,
        0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x11, 0x00, 0x00, 0x00, 0x10, 0x00,
        0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x02,
        0x00, 0x00, 0x06, 0x00, 0x03, 0x00, 0x06, 0x00, 0x03, 0x00, 0x06, 0x00, 0x03, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0xe4, 0xab, 0x00, 0x00,
        0x01, 0x00, 0x40, 0x05, 0x00, 0x00, 0x04, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x00, 0x10,
        0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3c, 0x30, 0x00, 0x00, 0x3c, 0x00, 0x00, 0x00, 0x00,
        0x40, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x1a, 0x00, 0x00, 0xb8, 0x22, 0x00, 0x00, 0x00, 0x50, 0x00, 0x00, 0x38, 0x00, 0x00,
        0x00, 0x10, 0x10, 0x00, 0x00, 0x38, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x68, 0x10, 0x00, 0x00, 0x5c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2e, 0x74, 0x65, 0x78, 0x74, 0x00, 0x00, 0x00, 0x24,
        0x06, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00,
        0x60, 0x2e, 0x64, 0x61, 0x74, 0x61, 0x00, 0x00, 0x00, 0x3c, 0x03, 0x00, 0x00, 0x00, 0x20,
        0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0xc0, 0x2e, 0x69, 0x64, 0x61,
        0x74, 0x61, 0x00, 0x00, 0xf8, 0x01, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x02, 0x00,
        0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x2e, 0x72, 0x73, 0x72, 0x63, 0x00, 0x00, 0x00, 0x00,
        0x08, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00,
        0x42, 0x2e, 0x72, 0x65, 0x6c, 0x6f, 0x63, 0x00, 0x00, 0x86, 0x01, 0x00, 0x00, 0x00, 0x50,
        0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn crss_header() {
        let header = Header::parse(&&CRSS_HEADER[..]).unwrap();
        assert!(header.dos_header.signature == DOS_MAGIC);
        assert!(header.signature == PE_MAGIC);
        assert!(header.coff_header.machine == COFF_MACHINE_X86);
        assert!(machine_to_str(header.coff_header.machine) == "X86");
        println!("header: {:?}", &header);
    }
}
