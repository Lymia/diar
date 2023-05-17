use crate::{
    compress::{dir_tree::DirNodeData, DirNode},
    errors::*,
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

pub static GEAR_TABLE: Table = [
    0xb9a737056bfa0e58, 0xfebb6c31b48737de, 0x01825746dcf248ca, 0x6fffaabd8522996b,
    0x4ca324402055c4bd, 0x8f465ffdbf614aed, 0x12ae835cc081b823, 0x3187e53de8fa2a14,
    0x1e63cf501c93443f, 0xeaae16ec062ed704, 0x4182f7021974f7d3, 0xdb43b8e4a89bc553,
    0x10e9bf57df5f242f, 0xcaf71497cbd3f859, 0x130d744853a7630c, 0x5188c175723ecacf,
    0x34554019af7e0326, 0x48b50cdd80001a0e, 0xffa6c88f16f0cd31, 0x0aca5583f24aa82a,
    0x51dcd5b38e809220, 0xbd90a91cceab52b0, 0x65d19b400bfd2c30, 0x568475953bbe0b1f,
    0xfa72b4a0499b0347, 0x2a3c0965a5700873, 0xff0b1cd0ea8a28e0, 0x213213f0295a266a,
    0x77e2c15472c3861b, 0xe03ebe3a030722f8, 0x56d1075054e4fcc5, 0xe88e30290f6dde5d,
    0xae85159fbe1914cb, 0xd64f8065152c8d76, 0x2d04562f42c6e7e3, 0x6e568ad31edc90bb,
    0x50738331ac3edeab, 0x76cd5f7c0213ac1e, 0xe27c4ec197610430, 0x4e4cd1c9105642c2,
    0x1705fdd350057155, 0x5b642f043a05704d, 0xd5486b0551dbc56b, 0x2727e6962491f57d,
    0x47d3c0d51aedd77a, 0xa78cf60c0347ad5f, 0x5872c2daf98342a2, 0xe13af30ff983ef32,
    0x2b7a6cea6af0f14d, 0xf4ba69ea44d10fc5, 0x508ecb58f8800d25, 0xe665da0c80e18a4e,
    0xdf3210a7aae96f1f, 0xe2ae8a5b4dc968c2, 0xb54b8f977301a80a, 0xc6cf30f22636fe15,
    0x21d08464f1960e64, 0xf0099cd8c04045d5, 0x8c3361baf80f6061, 0x63b3bf861975ca9c,
    0x7cf2279bc836e508, 0x99285b9596fac2cc, 0x18c10ac1c4f5aa13, 0x9a632dfd1f4f86fe,
    0x6aee4220299e5dd0, 0x4703d6d15b494a36, 0xb645da29e3f4f13f, 0x5301a07cc6e8f4b8,
    0xf16048603862b76f, 0x32ff8bad929ac9c8, 0xc31a3108140d80a5, 0x3bd0064efcbae83a,
    0xf1ca2cce6dbf3d1a, 0xb7405694c2081305, 0x6eed2a296187068d, 0x1bbdf4f532d0e250,
    0x49489306b05ab3ec, 0x6218de58d472ea03, 0x4dd6c337109518ba, 0x4a2114bf82ad19bc,
    0xbb42205bce9d5f8d, 0x61138afbb7f3cba6, 0x930ae05abe7205cf, 0xb02787da2366e14c,
    0x688e3d551d3b43c3, 0x914e65b48c312d01, 0x51d90557eba76c0b, 0x975a7a8788f2d0a8,
    0xb8534cd095a42e30, 0xeb2e37207d7a1de7, 0xea2d1ec7f70693bb, 0xb840b81ceec77fce,
    0x7502c7f926f24614, 0x16423f7f0634a205, 0x413073632d172044, 0x9e0272f2bb0a30eb,
    0x065a1aba3ed5da07, 0xf5f7b34a8b2b4806, 0xb03560ba69ca3b28, 0x28d752e633876dde,
    0x78309d82098cf5c4, 0xfa3a568da5feee3d, 0x4b03d632a07daa0d, 0xb1f321604f7dcffb,
    0x305325121ad6c2a0, 0xc28744e253229634, 0x40e38c525292bfd4, 0xe5772833a7242a1c,
    0xd3a1c359bc08e3ee, 0x42d0f71ba438608b, 0xd7d5d1a88065d019, 0x76dc2d917a1d59bd,
    0x934847043e1f8454, 0xe4d93518b3e7a18f, 0x46594d36fac65f16, 0x41bfece6bad9fefd,
    0x57dc47b8e880cbc1, 0xa85b440eb96ec469, 0x0db73f13feb80a12, 0x89616c963cbb993a,
    0xae73f24afb2d730e, 0x5720616995bdce0c, 0x9ece8b7d25697143, 0xfbfac69a94793dd9,
    0xc540f7baf28b1121, 0x3c901eab694e7870, 0x3026cbfad17d7bfc, 0x0144cfe6430abfa3,
    0xa9f9c7b9d6bda9a9, 0xcf468e8dba146bd9, 0xa3148a8639fe20f9, 0xc70ad6a3df2e4820,
    0x14199c8c41bbfa2a, 0x06331e16e513dff7, 0x4dbea7433062fe29, 0x2eeab725498ab321,
    0x67a4e82ebb3f17d5, 0x55f2654e5eebd927, 0x1bf142208169989b, 0x2680121ab958be87,
    0xfa311d8991a78747, 0x5d92163f46a5ab38, 0x0ff727e5e11efe3e, 0xd06590aa181c0541,
    0x39611a611d6f419d, 0xc04cd840c4641df0, 0x17fe47f92dcc3809, 0x119cd88ce137e3b3,
    0xae354e7a9a36365c, 0x09c54d1fbad60603, 0xea05e7f8e2b2c3c7, 0x658c4e2120f7a4f7,
    0xd4b84592313af71e, 0x733bfe1dbe371665, 0x3622f76295a656b9, 0x76dab1cafeda14ca,
    0x0307b3131e2e1ae5, 0xebf56345cb7a5f65, 0x237b5554dbdc347e, 0xae4cf7457e599ae3,
    0xaa168e0f28b8533f, 0x6b8ebfa9cc8977ee, 0x504b5082614815c4, 0x0c04d82070ff0585,
    0x2ae2235e3870f54e, 0x8d0e17804661719f, 0x08181c6beb88cbd2, 0xcbc6ee4b02f48488,
    0x3b412072b14622a4, 0xeb52ad91cb594daa, 0xcfa6dee526d5048e, 0xd9af8230a72defdd,
    0x8cb2b39959c552d2, 0xb1340e452085a458, 0x97b7962d089cc644, 0xaf547f02db295645,
    0x370bcd98618a0d68, 0x4136124a6fd2f97a, 0xb9218c1ea0e49c9f, 0x1dcf0dfc09f05dd8,
    0x6e26fd1c80344b4e, 0xa79a24c6e803c004, 0x8ba9d10f176ccec4, 0x799a8656bd4543c9,
    0xd0f1736b97c10eeb, 0x12f77aed1a0d1c57, 0xe132cb25d45d1de8, 0xf9841d6cb34cac1f,
    0x3313b57c6a239e3f, 0x030aecdd150f81a1, 0x7fa29f23276862be, 0x02d77a17af0a08a2,
    0xba36792bfc4bb7c1, 0x56b77ee4bd33a138, 0x055e3c690041cccb, 0xaba9d17dd6b4b837,
    0x5815be2ad817f813, 0xa1424a8bd479c2f0, 0x00bf7c8ec7ae782c, 0xd88963331f698684,
    0xc94f114cffd246ed, 0xa1aee7fb437850b4, 0x9b76865ebf88617f, 0xc7b254bc52fc3d3a,
    0x04bae6ea928b04a8, 0x80b41c2bf351ed42, 0xcfa6c19458dda9e0, 0xb3cb136defa8c009,
    0x43cc4cadb9ca03f0, 0x0e13f545629721f2, 0xbd0271b066f27c4d, 0xb9d09a9323f22254,
    0xd8d78bd43e43a7fb, 0xb0bd7500c541aa8e, 0x054edc5c7d1fe683, 0xeb782c921ae81eaa,
    0x3802b83398109abf, 0xc47ca348e8cf645c, 0x4df1c1112e93606a, 0x95272427edba117a,
    0xe1d161fac3084163, 0x254ed7dd0e78a249, 0x638b5a8f1b9bed03, 0x4959e75e3a95da66,
    0x527f5fa5610d5927, 0xa25ffe0666d292cc, 0xe230e84efec53318, 0x70ce34e11bb4c813,
    0x83a350a4210a232c, 0x3f8947a8bfe42153, 0x6f6e34c509565ed0, 0xdf75846abd8a5a83,
    0x73b223f04c23c967, 0x02561cc3806f2369, 0x8013b6ff8a483218, 0x71c1f4ecb5f81d0e,
    0xd8b1c941f3405ea8, 0x34b907f038f036ad, 0x3e2bf49c64e52eb6, 0x5c61fdcffc8c4114,
    0xd7e0a3541fbb7139, 0x98588d041d4c50e7, 0xf0417790d76b606f, 0x67ec0776c3888afe,
    0x789ca288272d213f, 0x16c45dfc16672f85, 0x98ec620decbef674, 0x72abcb3d02bf581a,
    0xc48cd9a0a3323579, 0xa6bfc83868383da0, 0x682b1cb48a2426bc, 0x6fb9951d609cd9e4,
    0x73b66ab2d88199c3, 0x1d1ef9c882ca3e43, 0xf4ba20802c76b06d, 0x44dc58750e0ee28c,
];

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
    let mut chunking = gearhash::Hasher::new(&GEAR_TABLE);

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
