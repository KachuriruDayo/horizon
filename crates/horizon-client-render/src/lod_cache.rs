//! The far terrain's samples on disk, so a world that is opened again does not sample its distant
//! tiles over (`lod::Lod` asks the generator for each tile's columns; that is the long part of a
//! far view's loading).
//!
//! One file per tile, a zlib-compressed list of its 16 by 16 columns, under
//! `<dir>/tiles/<detail><stride>/<region>/`. `<dir>/fingerprint` holds what the samples depend on
//! (the seed, the world preset, the dimension, the block registry, the format): when it differs
//! from the one asked for the tiles are dropped and sampled afresh. The cache holds the
//! generator's own terrain only. Chunks that were saved or visited replace it after it is read,
//! as they replace a sampled tile, so a world's edits still show.
use crate::lod::{Column, Span, Surface};
use horizon_world::registry::BlockStateId;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// What the files are written as: raise it when the encoding changes.
pub const FORMAT: u32 = 1;
const MAGIC: &[u8; 4] = b"HLD1";
/// The most a run writes (bytes): a cache that grows without end would fill a disk.
const WRITE_LIMIT: u64 = 3 << 30;

/// 64-bit FNV-1a: the same on every run and every compiler, unlike `DefaultHasher`.
pub fn fingerprint(parts: &[&[u8]]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for part in parts {
        for byte in part.iter().chain(&[0xff]) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

pub struct LodCache {
    dir: PathBuf,
    written: AtomicU64,
    /// Tiles read back, tiles asked for that were not there, tiles written (for the log).
    hits: AtomicU64,
    misses: AtomicU64,
    stored: AtomicU64,
}
impl Drop for LodCache {
    fn drop(&mut self) {
        log::info!(
            "Horizon: far terrain cache {}: {} tiles read from disk, {} sampled, {} kept ({} KiB)",
            self.dir.display(),
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
            self.stored.load(Ordering::Relaxed),
            self.written.load(Ordering::Relaxed) >> 10
        );
    }
}

impl LodCache {
    /// The cache in `dir`, made for samples that depend on `fingerprint`; tiles written for
    /// another fingerprint are removed. `None` where the folder can't be made, or when
    /// `HORIZON_LOD_CACHE=0`.
    pub fn open(dir: &Path, fingerprint: u64) -> Option<Arc<Self>> {
        if std::env::var("HORIZON_LOD_CACHE").is_ok_and(|v| v == "0") {
            return None;
        }
        fs::create_dir_all(dir).ok()?;
        let stamp = dir.join("fingerprint");
        let want = format!("{FORMAT}:{fingerprint:016x}");
        if fs::read_to_string(&stamp).ok().as_deref() != Some(want.as_str()) {
            let _ = fs::remove_dir_all(dir.join("tiles"));
            fs::write(&stamp, want).ok()?;
        }
        Some(Arc::new(Self {
            dir: dir.to_owned(),
            written: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            stored: AtomicU64::new(0),
        }))
    }

    fn path(&self, x: i32, z: i32, stride: i32, approximate: bool) -> PathBuf {
        self.dir
            .join("tiles")
            .join(format!("{}{stride}", if approximate { 'r' } else { 'f' }))
            .join(format!("{}_{}", x >> 10, z >> 10))
            .join(format!("{x}_{z}"))
    }

    /// The tile with its corner at `x`, `z` and `side` columns of `stride` blocks, if it was kept.
    pub fn load(
        &self,
        x: i32,
        z: i32,
        stride: i32,
        side: usize,
        approximate: bool,
    ) -> Option<Vec<Column>> {
        let read = || {
            let file = fs::File::open(self.path(x, z, stride, approximate)).ok()?;
            let mut bytes = Vec::new();
            flate2::read::ZlibDecoder::new(file)
                .read_to_end(&mut bytes)
                .ok()?;
            decode(&bytes, side * side)
        };
        let tile = read();
        let count = if tile.is_some() {
            &self.hits
        } else {
            &self.misses
        };
        count.fetch_add(1, Ordering::Relaxed);
        tile
    }

    /// Keeps the tile for the next run. Failures are dropped: it only costs the sampling again.
    pub fn store(&self, x: i32, z: i32, stride: i32, approximate: bool, columns: &[Column]) {
        if self.written.load(Ordering::Relaxed) >= WRITE_LIMIT {
            return;
        }
        let path = self.path(x, z, stride, approximate);
        let Some(parent) = path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        if encoder.write_all(&encode(columns)).is_err() {
            return;
        }
        let Ok(bytes) = encoder.finish() else {
            return;
        };
        // Written whole and moved into place: a tile half written when the game stops is never read.
        let temp = path.with_extension("tmp");
        if fs::write(&temp, &bytes).is_ok() && fs::rename(&temp, &path).is_ok() {
            self.written
                .fetch_add(bytes.len() as u64, Ordering::Relaxed);
            self.stored.fetch_add(1, Ordering::Relaxed);
        } else {
            let _ = fs::remove_file(&temp);
        }
    }
}

fn encode(columns: &[Column]) -> Vec<u8> {
    let mut out = Vec::with_capacity(columns.len() * 40);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(columns.len() as u32).to_le_bytes());
    for column in columns {
        match &column.surface {
            None => out.push(0),
            Some(s) => {
                out.push(1 + u8::from(s.water.is_some()));
                out.extend_from_slice(&s.height.to_le_bytes());
                out.extend_from_slice(&s.state.0.to_le_bytes());
                out.extend_from_slice(&s.biome.to_le_bytes());
                if let Some((height, state)) = s.water {
                    out.extend_from_slice(&height.to_le_bytes());
                    out.extend_from_slice(&state.0.to_le_bytes());
                }
            }
        }
        out.extend_from_slice(&(column.spans.len() as u16).to_le_bytes());
        for span in &column.spans {
            out.extend_from_slice(&span.min.to_le_bytes());
            out.extend_from_slice(&span.max.to_le_bytes());
            out.extend_from_slice(&span.state.0.to_le_bytes());
            out.extend_from_slice(&span.biome.to_le_bytes());
            out.extend_from_slice(&span.light.to_le_bytes());
        }
    }
    out
}

/// A cursor over bytes; every read fails (`None`) at the end instead of panicking.
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }
    fn i16(&mut self) -> Option<i16> {
        self.take().map(i16::from_le_bytes)
    }
    fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    fn i32(&mut self) -> Option<i32> {
        self.take().map(i32::from_le_bytes)
    }
}

fn decode(bytes: &[u8], expected: usize) -> Option<Vec<Column>> {
    let mut r = Reader(bytes);
    if &r.take::<4>()? != MAGIC || r.u32()? as usize != expected {
        return None;
    }
    let mut columns = Vec::with_capacity(expected);
    for _ in 0..expected {
        let kind = r.u8()?;
        let surface = if kind == 0 {
            None
        } else {
            let height = r.i32()?;
            let state = BlockStateId(r.u32()?);
            let biome = r.u16()?;
            let water = if kind == 2 {
                Some((r.i32()?, BlockStateId(r.u32()?)))
            } else {
                None
            };
            Some(Surface {
                height,
                state,
                water,
                biome,
            })
        };
        let count = r.u16()? as usize;
        let mut spans = Vec::with_capacity(count);
        for _ in 0..count {
            spans.push(Span {
                min: r.i16()?,
                max: r.i16()?,
                state: BlockStateId(r.u32()?),
                biome: r.u16()?,
                light: r.u16()?,
            });
        }
        columns.push(Column { surface, spans });
    }
    r.0.is_empty().then_some(columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Column> {
        (0..256)
            .map(|i| Column {
                surface: (i % 5 != 0).then_some(Surface {
                    height: 60 + i,
                    state: BlockStateId(i as u32 * 3),
                    water: (i % 3 == 0).then_some((62, BlockStateId(9))),
                    biome: (i % 7) as u16,
                }),
                spans: (0..(i % 4))
                    .map(|k| Span {
                        min: k as i16 * 4 - 8,
                        max: k as i16 * 4,
                        state: BlockStateId(k as u32 + 1),
                        biome: 2,
                        light: 0xF0,
                    })
                    .collect(),
            })
            .collect()
    }

    #[test]
    fn a_tile_comes_back_as_it_went() {
        let columns = sample();
        assert_eq!(decode(&encode(&columns), 256), Some(columns));
    }

    #[test]
    fn a_damaged_or_other_sized_tile_is_refused() {
        let bytes = encode(&sample());
        assert_eq!(decode(&bytes, 255), None);
        assert_eq!(decode(&bytes[..bytes.len() - 3], 256), None);
        assert_eq!(decode(b"nope", 256), None);
    }

    #[test]
    fn tiles_survive_a_reopen_and_a_new_fingerprint_drops_them() {
        let dir = std::env::temp_dir().join(format!("horizon-lod-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let columns = sample();
        let cache = LodCache::open(&dir, 7).unwrap();
        assert!(cache.load(-48, 1024, 2, 16, false).is_none());
        cache.store(-48, 1024, 2, false, &columns);
        assert_eq!(cache.load(-48, 1024, 2, 16, false), Some(columns.clone()));
        // Reduced detail has its own tiles.
        assert!(cache.load(-48, 1024, 2, 16, true).is_none());
        let again = LodCache::open(&dir, 7).unwrap();
        assert_eq!(again.load(-48, 1024, 2, 16, false), Some(columns));
        let other = LodCache::open(&dir, 8).unwrap();
        assert!(other.load(-48, 1024, 2, 16, false).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fingerprints_tell_their_parts_apart() {
        assert_ne!(fingerprint(&[b"ab", b"c"]), fingerprint(&[b"a", b"bc"]));
        assert_eq!(fingerprint(&[b"seed"]), fingerprint(&[b"seed"]));
    }
}
