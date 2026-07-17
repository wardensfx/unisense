//! Enumeration de process/modules et scan memoire (snapshot + diff) pour
//! l'assistant de detection automatique. Lecture seule, ReadProcessMemory
//! uniquement : rien n'est jamais ecrit dans un autre process.
//!
//! Le scan est volontairement borne (regions privees lisibles/inscriptibles
//! seulement, plafond de taille totale) : c'est un scan "premiere passe" a
//! la Cheat Engine, pas un outil de reverse engineering complet.

use std::collections::HashMap;

use serde::Serialize;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
    MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_PRIVATE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

/// Plafond du volume total lu par snapshot : garde le scan rapide et leger,
/// suffisant pour reperer un octet d'etat menu/gameplay dans une zone de
/// heap "petites variables" typique.
const MAX_TOTAL_SCAN_BYTES: usize = 96 * 1024 * 1024;
/// Ignore les regions individuelles plus grosses que ca (gros buffers,
/// textures, etc. : rarement la ou vit un flag d'etat, couteux a diff).
const MAX_REGION_BYTES: usize = 4 * 1024 * 1024;
/// Nombre maximum de candidats renvoyes par un diff (au-dela, l'utilisateur
/// doit affiner : changer d'etat plus franchement, refaire un snapshot).
const MAX_DIFF_RESULTS: usize = 500;

struct HandleGuard(HANDLE);
impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[derive(Serialize, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
}

pub fn list_processes() -> anyhow::Result<Vec<ProcessEntry>> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let _guard = HandleGuard(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut out = Vec::new();
        if Process32FirstW(snapshot, &mut entry).is_err() {
            return Ok(out);
        }
        loop {
            let name = wide_to_string(&entry.szExeFile);
            if !name.is_empty() {
                out.push(ProcessEntry {
                    pid: entry.th32ProcessID,
                    name,
                });
            }
            if Process32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }
        out.sort_by_key(|a| a.name.to_lowercase());
        out.dedup_by(|a, b| a.name == b.name && a.pid == b.pid);
        Ok(out)
    }
}

pub fn list_modules(pid: u32) -> anyhow::Result<Vec<String>> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)?;
        let _guard = HandleGuard(snapshot);
        let mut entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        let mut out = Vec::new();
        if Module32FirstW(snapshot, &mut entry).is_err() {
            return Ok(out);
        }
        loop {
            out.push(wide_to_string(&entry.szModule));
            if Module32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }
        Ok(out)
    }
}

/// Une region memoire capturee (adresse de base + octets lus).
pub struct Region {
    pub base: u64,
    pub bytes: Vec<u8>,
}

pub struct Snapshot {
    pub regions: Vec<Region>,
    /// Modules charges au moment du snapshot, pour resoudre adresse -> (module, offset).
    pub modules: Vec<(String, u64, u64)>, // (nom, base, taille)
}

pub fn take_snapshot(pid: u32) -> anyhow::Result<Snapshot> {
    let modules = list_modules_with_base(pid)?;

    unsafe {
        let process = OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid)?;
        let _guard = HandleGuard(process);

        let mut regions = Vec::new();
        let mut address: u64 = 0;
        let mut total_scanned: usize = 0;

        loop {
            let mut mbi = MEMORY_BASIC_INFORMATION::default();
            let written = VirtualQueryEx(
                process,
                Some(address as *const _),
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            );
            if written == 0 {
                break;
            }

            let region_size = mbi.RegionSize as u64;
            let is_scannable = mbi.State == MEM_COMMIT
                && mbi.Type == MEM_PRIVATE
                && mbi.Protect == PAGE_READWRITE
                && mbi.RegionSize > 0
                && mbi.RegionSize <= MAX_REGION_BYTES;

            if is_scannable && total_scanned < MAX_TOTAL_SCAN_BYTES {
                let base = mbi.BaseAddress as u64;
                let mut buf = vec![0u8; mbi.RegionSize];
                let mut read = 0usize;
                let ok = ReadProcessMemory(
                    process,
                    base as *const _,
                    buf.as_mut_ptr() as *mut _,
                    buf.len(),
                    Some(&mut read as *mut usize),
                )
                .is_ok();
                if ok && read == buf.len() {
                    total_scanned += buf.len();
                    regions.push(Region { base, bytes: buf });
                }
            }

            let next = (mbi.BaseAddress as u64).saturating_add(region_size.max(1));
            if next <= address {
                break; // protection anti boucle infinie
            }
            address = next;
            if total_scanned >= MAX_TOTAL_SCAN_BYTES {
                break;
            }
        }

        Ok(Snapshot { regions, modules })
    }
}

#[derive(Serialize, Clone)]
pub struct DiffEntry {
    pub address_hex: String,
    pub module: Option<String>,
    pub offset_hex: Option<String>,
    pub value_a: u8,
    pub value_b: u8,
}

pub fn diff_snapshots(a: &Snapshot, b: &Snapshot) -> Vec<DiffEntry> {
    let b_by_base: HashMap<u64, &Region> = b.regions.iter().map(|r| (r.base, r)).collect();
    let mut out = Vec::new();

    'regions: for ra in &a.regions {
        let Some(rb) = b_by_base.get(&ra.base) else {
            continue;
        };
        if rb.bytes.len() != ra.bytes.len() {
            continue;
        }
        for (i, (&va, &vb)) in ra.bytes.iter().zip(rb.bytes.iter()).enumerate() {
            if va != vb {
                let addr = ra.base + i as u64;
                let (module, offset) = resolve_module(&b.modules, addr);
                out.push(DiffEntry {
                    address_hex: format!("0x{addr:X}"),
                    module,
                    offset_hex: offset.map(|o| format!("0x{o:X}")),
                    value_a: va,
                    value_b: vb,
                });
                if out.len() >= MAX_DIFF_RESULTS {
                    break 'regions;
                }
            }
        }
    }
    out
}

fn resolve_module(modules: &[(String, u64, u64)], addr: u64) -> (Option<String>, Option<u64>) {
    for (name, base, size) in modules {
        if addr >= *base && addr < base + size {
            return (Some(name.clone()), Some(addr - base));
        }
    }
    (None, None)
}

fn list_modules_with_base(pid: u32) -> anyhow::Result<Vec<(String, u64, u64)>> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)?;
        let _guard = HandleGuard(snapshot);
        let mut entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        let mut out = Vec::new();
        if Module32FirstW(snapshot, &mut entry).is_err() {
            return Ok(out);
        }
        loop {
            out.push((
                wide_to_string(&entry.szModule),
                entry.modBaseAddr as u64,
                entry.modBaseSize as u64,
            ));
            if Module32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }
        Ok(out)
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}
