use crate::{
    errors::*,
    writer::{dir_tree::DirNodeData, DirNode},
};
use derive_setters::Setters;
use gearhash::Table;
use priority_queue::DoublePriorityQueue;
use std::{
    borrow::Borrow,
    cmp,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use twox_hash::Xxh3Hash64;

#[derive(Copy, Clone, Debug, Default, Setters)]
#[non_exhaustive]
pub struct ChunkConfig {
    mask: u64,
    min: usize,
    max: usize,
}
impl ChunkConfig {
    pub fn new(mask: u64, min: usize, max: usize) -> ChunkConfig {
        ChunkConfig { mask, min, max }
    }
}

#[derive(Copy, Clone, Debug, Setters)]
#[non_exhaustive]
pub struct BuildSamplesConfiguration {
    pub dictionary_size: usize,
    pub excess_samples_factor: usize,
    pub hash_table_size: usize,
    pub chunker: ChunkConfig,

    pub basic_dict_samples_count: usize,
    pub basic_dict_samples_max_size: usize,
}
impl Default for BuildSamplesConfiguration {
    fn default() -> Self {
        BuildSamplesConfiguration {
            dictionary_size: 1024 * 512, // 512 KiB dictionary
            excess_samples_factor: 2,
            hash_table_size: 1024 * 1024 * 64, // 512 MiB total size
            chunker: ChunkConfig::new(0x0000000000008835, 16, 256), // avg 1/64 chance
            basic_dict_samples_count: 128,
            basic_dict_samples_max_size: 1024 * 32,
        }
    }
}

struct PriorityCell {
    hash: u64,
    data: Vec<u8>,
    data_size: usize,
}
impl Hash for PriorityCell {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}
impl PartialEq for PriorityCell {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl Eq for PriorityCell {}
impl Borrow<u64> for PriorityCell {
    fn borrow(&self) -> &u64 {
        &self.hash
    }
}

fn do_chunk(cfg: ChunkConfig, data: &[u8], mut callback: impl FnMut(&[u8])) {
    let mut chunking = gearhash::Hasher::new(&crate::GEAR_TABLE);

    let mut chunk_start = 0;
    let mut start = 0;
    loop {
        if start == data.len() {
            return;
        }

        let max_count = cmp::min(data.len() - start, cfg.max);
        let count = chunking.next_match(&data[start..start + max_count], cfg.mask);
        let end = match count {
            Some(x) => start + x,
            None => start + max_count,
        };

        let chunk = &data[chunk_start..end];
        if chunk.len() < cfg.min {
            start = end;
        } else {
            callback(chunk);
            chunk_start = end;
            start = end;
        }
    }
}

struct ChunkBuilder {
    priority_queue: DoublePriorityQueue<PriorityCell, u64>,
    alloc_cells: Vec<PriorityCell>,
    hash_count: Vec<u64>,
    chunker: ChunkConfig,
}
impl ChunkBuilder {
    fn new(cfg: &BuildSamplesConfiguration) -> Self {
        let mut alloc_cells = Vec::new();
        let needed_samples = cfg.dictionary_size / cfg.chunker.min;
        for _ in 0..needed_samples * cfg.excess_samples_factor {
            alloc_cells.push(PriorityCell {
                hash: 0,
                data: vec![0; cfg.chunker.max],
                data_size: 0,
            });
        }

        ChunkBuilder {
            priority_queue: DoublePriorityQueue::new(),
            alloc_cells,
            hash_count: vec![0; cfg.hash_table_size],
            chunker: cfg.chunker,
        }
    }

    fn push_chunk(&mut self, mask: u64, chunk: &[u8]) {
        let (hash, count) = {
            // generate 3 different hashes from the input data chunk; we don't use gear here
            let mut hasher = Xxh3Hash64::with_seed(mask.wrapping_mul(0x092887b6049aa1fd));
            hasher.write(chunk);
            let mut hash_a = hasher.finish();

            let mut hasher = DefaultHasher::new();
            hasher.write_u64(hash_a ^ 0x13b75835cec06997);
            let mut hash_b = hasher.finish();
            hasher.write_u64(hash_a ^ 0x907c1340fc4f2ba7);
            let mut hash_c = hasher.finish();

            // find the minimum hash count among the three
            macro_rules! bloom_insert {
                ($hash:expr) => {{
                    let idx = ($hash as usize) % self.hash_count.len();
                    self.hash_count[idx] += chunk.len() as u64;
                    self.hash_count[idx]
                }};
            }
            let ct_a = bloom_insert!(hash_a);
            let ct_b = bloom_insert!(hash_b);
            let ct_c = bloom_insert!(hash_c);

            (hash_a, cmp::min(ct_a, cmp::min(ct_b, ct_c)))
        };

        // just change the hash count if it's already in the queue
        if self.priority_queue.get(&hash).is_some() {
            self.priority_queue.change_priority(&hash, count);
            return;
        }

        // bail if the count is less than the highest in the priority queue
        if let Some((_, &min_prio)) = self.priority_queue.peek_min() {
            if count <= min_prio {
                return;
            }
        }

        // insert the new cell into the queue
        let mut new_cell = self
            .alloc_cells
            .pop()
            .unwrap_or_else(|| self.priority_queue.pop_min().unwrap().0);
        new_cell.hash = hash;
        new_cell.data_size = cmp::min(chunk.len(), new_cell.data.len());
        new_cell.data[..new_cell.data_size].copy_from_slice(&chunk[..new_cell.data_size]);
        self.priority_queue.push(new_cell, count);
    }

    fn push_file(&mut self, data: &[u8]) {
        do_chunk(self.chunker, data, |x| self.push_chunk(self.chunker.mask, x));
    }

    fn build_dictionary(mut self, max_size: usize) -> Vec<u8> {
        drop((self.alloc_cells, self.hash_count));

        let mut dictionary_data = Vec::new();
        while let Some((chunk, usage)) = self.priority_queue.pop_max() {
            dictionary_data.extend(&chunk.data);
            if dictionary_data.len() >= max_size {
                break;
            }
        }
        dictionary_data.truncate(max_size);
        dictionary_data.shrink_to_fit();
        dictionary_data
    }
}

struct SamplesBuilder {
    samples: Vec<Vec<u8>>,
    hash: Xxh3Hash64,
    processed: usize,

    samples_count: usize,
    samples_max_size: usize,
}
impl SamplesBuilder {
    fn new(cfg: &BuildSamplesConfiguration) -> SamplesBuilder {
        SamplesBuilder {
            samples: vec![],
            hash: Xxh3Hash64::with_seed(1234),
            processed: 0,
            samples_count: cfg.basic_dict_samples_count,
            samples_max_size: cfg.basic_dict_samples_max_size,
        }
    }

    fn push_sample(&mut self, data: &[u8]) {
        self.hash.write(&data[..cmp::min(1024, data.len())]);

        if self.samples.len() < self.samples_count {
            let new_data = data[..cmp::min(self.samples_max_size, data.len())].to_vec();
            self.samples.push(new_data);
        } else if (self.hash.finish() as usize % self.processed) < self.samples_count {
            let new_data = data[..cmp::min(self.samples_max_size, data.len())].to_vec();
            let idx = (self.hash.finish() >> 32) as usize % self.samples.len();
            self.samples[idx] = new_data;
        }

        self.processed += 1;
    }

    fn build_dictionary(mut self, max_size: usize) -> Result<Vec<u8>> {
        Ok(zstd::dict::from_samples(&self.samples, max_size)?)
    }
}

pub struct BuildSamples {
    chunk_builder: ChunkBuilder,
    samples_builder: SamplesBuilder,
    target_size: usize,
}
impl BuildSamples {
    pub fn new(cfg: &BuildSamplesConfiguration) -> Self {
        BuildSamples {
            chunk_builder: ChunkBuilder::new(cfg),
            samples_builder: SamplesBuilder::new(cfg),
            target_size: cfg.dictionary_size,
        }
    }

    pub fn push_file(&mut self, data: &[u8]) {
        self.chunk_builder.push_file(data);
        self.samples_builder.push_sample(data);
    }
    pub fn add_nodes(&mut self, node: &DirNode) -> Result<&mut Self> {
        match &node.data {
            DirNodeData::FileNode { contents, .. } => {
                let mut data = Vec::new();
                contents.push_to_vec(&mut data)?;
                self.push_file(&data);
            }
            DirNodeData::DirNode { contents, .. } => {
                for node in contents.values() {
                    self.add_nodes(node)?;
                }
            }
        }
        Ok(self)
    }

    pub fn build_dictionary(self) -> Result<Vec<u8>> {
        let mut new_dict = self.samples_builder.build_dictionary(256)?;
        new_dict.extend(self.chunk_builder.build_dictionary(self.target_size));
        new_dict.truncate(self.target_size);
        Ok(new_dict)
    }
}
