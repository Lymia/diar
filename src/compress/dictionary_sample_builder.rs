pub struct BuildSamplesConfiguration {
    pub total_size: usize,
    pub min_chunk_size: usize,
    pub bucket_count: usize,
    pub hash_bucket_factor: usize,
}
impl Default for BuildSamplesConfiguration {
    fn default() -> Self {
        BuildSamplesConfiguration {
            total_size: 1024 * 1024 * 4, // 4 MB
            min_chunk_size: 1024,
            bucket_count: 4,
            hash_bucket_factor: 512,
        }
    }
}

pub struct BuildSamples {
    min_bucket_chunk_size: usize,
    bucket_size: usize,
    bucket_count: usize,

    hash_count: Vec<usize>,
    sample_max_count: Vec<Vec<usize>>,
    sample_data: Vec<u8>,
}
impl BuildSamples {
    pub fn new(cfg: &BuildSamplesConfiguration) -> Self {
        let bucket_size = cfg.total_size / cfg.bucket_count;
        assert!(cfg.total_size % cfg.bucket_count == 0);
        assert!(bucket_size % cfg.min_chunk_size == 0);

        let hash_bucket_count = (cfg.total_size / cfg.min_chunk_size) * cfg.hash_bucket_factor;

        let mut sample_max_count = Vec::new();

        let mut cur_chunk_size = cfg.min_chunk_size;
        let mut cur_count = (cfg.total_size / cfg.bucket_count) / cfg.min_chunk_size;
        for _ in 0..cfg.bucket_count {
            assert_eq!(cur_chunk_size * count, bucket_size);

            sample_max_count.push(vec![0; count]);

            cur_chunk_size *= 2;
            count /= 2;
        }

        BuildSamples {
            min_bucket_chunk_size: cfg.min_chunk_size,
            bucket_size: cfg.total_size / cfg.bucket_count,
            bucket_count: cfg.bucket_count,
            hash_count: vec![0; hash_bucket_count],
            sample_max_count,
            sample_data: vec![0; cfg.total_size],
        }
    }
}
