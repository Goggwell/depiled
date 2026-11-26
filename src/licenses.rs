//! Which apps the account owns, from files Steam keeps on disk: the account's license list
//! (`userdata/<id>/config/licensecache`) and the package contents (`appcache/packageinfo.vdf`).

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::vdf::{BinaryReader, Value};

const PACKAGES_V27: u32 = 0x0656_5527;
const PACKAGES_V28: u32 = 0x0656_5528;

#[derive(Debug, Clone, Copy)]
pub struct License {
    pub package: u32,
    pub flags: u32,
    /// Account that owns the license; another account for games shared by family.
    pub owner: u32,
}

/// Licenses that are expired, cancelled or not activated.
const INACTIVE: u32 = 8 | 16 | 32 | 1024 | 2048;

/// Apps the account owns, and apps lent to it by its Steam Family.
pub struct Ownership {
    pub owned: BTreeSet<u32>,
    pub family: BTreeSet<u32>,
}

/// Every app id included in a package the account holds an active license for.
pub fn owned_apps(steam_dir: &Path, account_id: u32) -> Result<BTreeSet<u32>> {
    Ok(ownership(steam_dir, account_id)?.owned)
}

pub fn ownership(steam_dir: &Path, account_id: u32) -> Result<Ownership> {
    let license_file = steam_dir.join("userdata").join(account_id.to_string()).join("config").join("licensecache");
    let licenses = read_licenses(&std::fs::read(&license_file).with_context(|| format!("reading {}", license_file.display()))?, account_id)?;
    let package_file = steam_dir.join("appcache").join("packageinfo.vdf");
    let packages = package_apps(&std::fs::read(&package_file).with_context(|| format!("reading {}", package_file.display()))?)?;
    let apps = |family: bool| -> BTreeSet<u32> {
        licenses
            .iter()
            // Family-shared licenses belong to someone else in the account's Steam Family.
            .filter(|l| l.package != 0 && l.flags & INACTIVE == 0 && (l.owner != account_id) == family)
            .filter_map(|l| packages.get(&l.package))
            .flatten()
            .copied()
            .collect()
    };
    let owned = apps(false);
    let family = apps(true).difference(&owned).copied().collect();
    Ok(Ownership { owned, family })
}

/// The license cache is a protobuf message XOR-ed with a byte stream seeded by the account id,
/// followed by a 4-byte checksum.
pub fn read_licenses(data: &[u8], account_id: u32) -> Result<Vec<License>> {
    if data.len() <= 4 {
        return Ok(Vec::new());
    }
    let mut stream = ByteStream::seeded(account_id as i32);
    let plain: Vec<u8> = data[..data.len() - 4].iter().map(|b| b ^ stream.next_char()).collect();

    let mut out = Vec::new();
    let mut pos = 0;
    while pos < plain.len() {
        let (field, wire) = tag(&plain, &mut pos)?;
        if field == 2 && wire == 2 {
            let len = varint(&plain, &mut pos)? as usize;
            let body = plain.get(pos..pos + len).context("license entry is cut off")?;
            pos += len;
            let mut license = License { package: 0, flags: 0, owner: account_id };
            let mut p = 0;
            while p < body.len() {
                let (f, w) = tag(body, &mut p)?;
                match (f, w) {
                    (1, 0) => license.package = varint(body, &mut p)? as u32,
                    (7, 0) => license.flags = varint(body, &mut p)? as u32,
                    (12, 0) => license.owner = varint(body, &mut p)? as u32,
                    _ => skip(body, &mut p, w)?,
                }
            }
            out.push(license);
        } else {
            skip(&plain, &mut pos, wire)?;
        }
    }
    Ok(out)
}

fn tag(data: &[u8], pos: &mut usize) -> Result<(u64, u64)> {
    let key = varint(data, pos)?;
    Ok((key >> 3, key & 7))
}

fn varint(data: &[u8], pos: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *data.get(*pos).context("number is cut off")?;
        *pos += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    bail!("number is too long")
}

fn skip(data: &[u8], pos: &mut usize, wire: u64) -> Result<()> {
    match wire {
        0 => {
            varint(data, pos)?;
        }
        1 => *pos += 8,
        2 => *pos += varint(data, pos)? as usize,
        5 => *pos += 4,
        other => bail!("unknown protobuf wire type {other}"),
    }
    if *pos > data.len() {
        bail!("field runs past the end");
    }
    Ok(())
}

/// Source engine's uniform random stream (Park-Miller with a shuffle table), which Steam uses
/// to mask the license cache.
struct ByteStream {
    idum: i32,
    iy: i32,
    table: [i32; 32],
}

impl ByteStream {
    const IA: i32 = 16807;
    const IM: i32 = i32::MAX;
    const IQ: i32 = 127_773;
    const IR: i32 = 2836;
    const NDIV: i32 = 1 + (Self::IM - 1) / 32;

    fn seeded(seed: i32) -> Self {
        Self { idum: if seed < 0 { seed } else { -seed }, iy: 0, table: [0; 32] }
    }

    fn step(&mut self) {
        let k = self.idum / Self::IQ;
        self.idum = Self::IA * (self.idum - k * Self::IQ) - Self::IR * k;
        if self.idum < 0 {
            self.idum += Self::IM;
        }
    }

    fn next(&mut self) -> i32 {
        if self.idum <= 0 || self.iy == 0 {
            self.idum = (-self.idum).max(1);
            for j in (0..=32 + 7).rev() {
                self.step();
                if j < 32 {
                    self.table[j] = self.idum;
                }
            }
            self.iy = self.table[0];
        }
        self.step();
        let mut j = self.iy / Self::NDIV;
        if !(0..32).contains(&j) {
            j = (j % 32) & i32::MAX;
        }
        self.iy = self.table[j as usize];
        self.table[j as usize] = self.idum;
        self.iy
    }

    /// A value in 32..=126, drawn without modulo bias like the engine does.
    fn next_char(&mut self) -> u8 {
        const RANGE: u32 = 126 - 32 + 1;
        const MAX: u32 = i32::MAX as u32;
        let accept = MAX - ((MAX + 1) % RANGE);
        loop {
            let v = self.next() as u32;
            if v <= accept {
                return (32 + v % RANGE) as u8;
            }
        }
    }
}

/// Package id to the app ids it contains.
pub fn package_apps(data: &[u8]) -> Result<HashMap<u32, Vec<u32>>> {
    let mut r = BinaryReader::new(data);
    let magic = r.u32()?;
    if magic != PACKAGES_V27 && magic != PACKAGES_V28 {
        bail!("packageinfo.vdf has an unknown format (0x{magic:08x})");
    }
    let _universe = r.u32()?;
    let mut out = HashMap::new();
    loop {
        let id = r.u32()?;
        if id == u32::MAX {
            break;
        }
        r.bytes(20 + 4)?; // sha1, change number
        if magic == PACKAGES_V28 {
            r.u64()?; // access token
        }
        let root = Value::Obj(r.object()?);
        // One object named after the package id holds the details.
        let apps = root
            .entries()
            .first()
            .and_then(|(_, pkg)| pkg.get("appids"))
            .map(|a| a.entries().iter().filter_map(|(_, v)| v.as_i64()).map(|v| v as u32).collect())
            .unwrap_or_default();
        out.insert(id, apps);
    }
    Ok(out)
}

