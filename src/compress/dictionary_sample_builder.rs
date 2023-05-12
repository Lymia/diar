use crate::compress::dir_tree::DirNodeData;
use crate::compress::DirNode;
use crate::errors::*;
use fastcdc::v2020::FastCDC;
use std::ffi::CStr;
use std::hash::Hasher;
use std::io::ErrorKind;

#[non_exhaustive]
pub struct BuildSamplesConfiguration {
    pub total_size: usize,
    pub min_chunk_size: usize,
    pub bucket_count: usize,
    pub hash_bucket_factor: usize,
    pub dupe_factor: usize,

    pub chunking_min: u32,
    pub chunking_avg: u32,
    pub chunking_max: u32,
}
impl Default for BuildSamplesConfiguration {
    fn default() -> Self {
        BuildSamplesConfiguration {
            total_size: 1024 * 1024 * 10,
            min_chunk_size: 64,
            bucket_count: 5,
            hash_bucket_factor: 256,
            dupe_factor: 8,
            chunking_min: 64,
            chunking_avg: 320,
            chunking_max: 1280,
        }
    }
}

pub struct BuildSamples {
    min_bucket_chunk_size: usize,
    max_bucket_chunk_count: usize,
    bucket_size: usize,
    bucket_count: usize,
    dupe_factor: usize,

    chunking_min: u32,
    chunking_avg: u32,
    chunking_max: u32,

    hash_count: Vec<usize>,
    sample_max_count: Vec<Vec<usize>>,
    sample_data: Vec<u8>,
}
impl BuildSamples {
    pub fn new(cfg: &BuildSamplesConfiguration) -> Self {
        assert!(cfg.total_size > 0);
        assert!(cfg.min_chunk_size > 0);
        assert!(cfg.bucket_count > 0);
        assert!(cfg.hash_bucket_factor > 0);
        assert!(cfg.dupe_factor > 0);

        let bucket_size = cfg.total_size / cfg.bucket_count;
        assert!(cfg.total_size % cfg.bucket_count == 0);
        assert!(bucket_size % cfg.min_chunk_size == 0);

        let hash_bucket_count = (cfg.total_size / cfg.min_chunk_size) * cfg.hash_bucket_factor;

        let mut sample_max_count = Vec::new();

        let mut cur_chunk_size = cfg.min_chunk_size;
        let mut cur_count = (cfg.total_size / cfg.bucket_count) / cfg.min_chunk_size;
        for _ in 0..cfg.bucket_count {
            assert_eq!(cur_chunk_size * cur_count, bucket_size);

            sample_max_count.push(vec![0; cur_count]);

            cur_chunk_size *= 2;
            cur_count /= 2;
        }

        BuildSamples {
            min_bucket_chunk_size: cfg.min_chunk_size,
            max_bucket_chunk_count: bucket_size / cfg.min_chunk_size,
            bucket_size,
            bucket_count: cfg.bucket_count,
            dupe_factor: cfg.dupe_factor,
            chunking_min: cfg.chunking_min,
            chunking_avg: cfg.chunking_avg,
            chunking_max: cfg.chunking_max,
            hash_count: vec![0; hash_bucket_count],
            sample_max_count,
            sample_data: vec![0; cfg.total_size],
        }
    }

    fn push_sample(&mut self, hash: u64, sample: &[u8]) {
        if sample.len() >= self.min_bucket_chunk_size {
            // whiten the hash from the raw hasher
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            hasher.write_u64(hash);
            let hash = hasher.finish();

            // update the hash count
            let idx = hash as usize % self.hash_count.len();
            self.hash_count[idx] += 1;
            let hash_count = self.hash_count[idx];

            // find the appropriate bucket for this sample.
            let mut bucket = 0;
            let mut bucket_chunk_size = self.min_bucket_chunk_size;
            let mut bucket_chunk_count = self.max_bucket_chunk_count;
            while bucket != self.bucket_count - 1 && bucket_chunk_size * 2 <= sample.len() {
                bucket += 1;
                bucket_chunk_size *= 2;
                bucket_chunk_count /= 2;
            }

            // check if we should write this sample.
            let idx = hash as usize % bucket_chunk_count;
            if hash_count > self.sample_max_count[bucket][idx] {
                self.sample_max_count[bucket][idx] = hash_count;

                let start_idx = self.bucket_size * bucket + idx * bucket_chunk_size;
                self.sample_data[start_idx..start_idx + bucket_chunk_size]
                    .copy_from_slice(&sample[..bucket_chunk_size]);
            }
        }
    }

    pub fn push_file(&mut self, data: &[u8]) {
        let mut chunking =
            FastCDC::new(data, self.chunking_min, self.chunking_avg, self.chunking_max);

        let mut start = 0;
        while start != data.len() {
            let (hash, new_start) = chunking.cut(start, data.len() - start);
            for i in 0..self.dupe_factor {
                self.push_sample(hash.wrapping_add(i as u64 * 123), &data[start..new_start]);
            }
            start = new_start;
        }
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
    pub fn build_dictionary(&self, max_size: usize) -> Result<Vec<u8>> {
        let mut sizes = Vec::new();

        let mut bucket_chunk_size = self.min_bucket_chunk_size;
        let mut bucket_chunk_count = self.max_bucket_chunk_count;
        for _ in 0..self.bucket_count {
            for _ in 0..bucket_chunk_count {
                sizes.push(bucket_chunk_size);
            }

            bucket_chunk_size *= 2;
            bucket_chunk_count /= 2;
        }

        std::fs::write("dict/raw_samples.bin", &self.sample_data)?;
        debug!("{:?}", self.sample_max_count);
        Ok(train_from_continuous(&self.sample_data, &sizes, max_size)?)
    }
}

fn train_from_continuous(linear: &[u8], sizes: &[usize], max_size: usize) -> Result<Vec<u8>> {
    unsafe {
        let mut data: Vec<u8> = Vec::with_capacity(max_size);
        data.set_len(max_size);

        assert_eq!(sizes.iter().sum::<usize>(), linear.len());
        let result = zstd_sys::ZDICT_trainFromBuffer_fastCover(
            data.as_mut_ptr() as *mut _,
            data.len(),
            linear.as_ptr() as *const _,
            sizes.as_ptr(),
            sizes.len() as u32,
            zstd_sys::ZDICT_fastCover_params_t {
                k: 20,
                d: 8,
                f: 26,
                steps: 0,
                nbThreads: num_cpus::get() as u32,
                splitPoint: 0.85,
                accel: 2,
                shrinkDict: 0,
                shrinkDictMaxRegression: 0,
                zParams: zstd_sys::ZDICT_params_t {
                    compressionLevel: 0,
                    notificationLevel: 0,
                    dictID: 0,
                },
            },
        );
        if zstd_sys::ZSTD_isError(result) != 0 {
            let cstr = CStr::from_ptr(zstd_sys::ZSTD_getErrorName(result));
            let err = cstr.to_str().unwrap();
            Err(std::io::Error::new(ErrorKind::Other, err).into())
        } else {
            data.set_len(result);
            data.shrink_to_fit();
            Ok(data)
        }
    }
}
