//! Shared parse-tree types and the top-level `parse` entry point.
//!
//! Owned by the orchestrator, not any single bead, so the three parser
//! modules (header/samples/patterns) can be implemented concurrently
//! against a fixed contract without touching this file.

pub mod header;
pub mod patterns;
pub mod samples;

use serde::Serialize;

pub const SAMPLE_COUNT: usize = 31;
pub const ROWS_PER_PATTERN: usize = 64;
pub const CHANNELS: usize = 4;

#[derive(Debug, Clone, Serialize)]
pub struct Header {
    pub name: String,
    pub song_length: u8,
    pub restart_position: u8,
    pub order: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    pub name: String,
    pub length_bytes: u32,
    pub finetune: i8,
    pub volume: u8,
    pub loop_start_bytes: u32,
    pub loop_len_bytes: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Cell {
    pub sample: u8,
    pub period: u16,
    pub effect: u8,
    pub param: u8,
}

pub type Pattern = Vec<[Cell; CHANNELS]>;

#[derive(Debug, Clone, Serialize)]
pub struct Module {
    pub header: Header,
    pub samples: Vec<Sample>,
    pub patterns: Vec<Pattern>,
}

/// Byte offset of the "M.K." tag: 20 (name) + 31 * 30 (sample headers).
pub const TAG_OFFSET: usize = 20 + SAMPLE_COUNT * 30;
/// Byte offset where pattern data begins: tag offset + 4-byte tag.
pub const PATTERN_DATA_OFFSET: usize = TAG_OFFSET + 4;

pub fn parse(bytes: &[u8]) -> anyhow::Result<Module> {
    let header = header::parse(bytes)?;
    let samples = samples::parse(bytes)?;
    let patterns = patterns::parse(bytes, &header.order, header.song_length)?;
    Ok(Module { header, samples, patterns })
}
