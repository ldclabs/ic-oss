pub mod store;
pub mod types;

#[cfg(test)]
mod test {

    use ic_stable_structures::{
        memory_manager::{MemoryId, MemoryManager, VirtualMemory},
        DefaultMemoryImpl, StableBTreeMap,
    };
    use std::cell::RefCell;

    use crate::ic_oss_fs;
    use crate::types::{Chunk, FileId, FileMetadata};

    type Memory = VirtualMemory<DefaultMemoryImpl>;

    const FS_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);

    thread_local! {

        static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
            RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));


        // `FS_CHUNKS_STORE`` is needed by `ic_oss_can::ic_oss_fs` macro
        static FS_CHUNKS_STORE: RefCell<StableBTreeMap<FileId, Chunk, Memory>> = RefCell::new(
            StableBTreeMap::init(
                MEMORY_MANAGER.with_borrow(|m| m.get(FS_DATA_MEMORY_ID)),
            )
        );
    }

    // need to define `FS_CHUNKS_STORE` before `ic_oss_can::ic_oss_fs!()`
    ic_oss_fs!();

    #[test]
    fn test_ic_oss_fs() {
        let files = fs::list_files(u32::MAX, 2);
        assert!(files.is_empty());

        fs::add_file(FileMetadata {
            name: "f1".to_string(),
            size: 100,
            ..Default::default()
        })
        .unwrap();

        assert!(fs::get_file(0).is_none());
        assert_eq!(fs::get_file(1).unwrap().name, "f1");

        fs::add_file(FileMetadata {
            name: "f2".to_string(),
            size: 100,
            ..Default::default()
        })
        .unwrap();

        fs::add_file(FileMetadata {
            name: "f3".to_string(),
            size: 100,
            ..Default::default()
        })
        .unwrap();

        fs::add_file(FileMetadata {
            name: "f4".to_string(),
            size: 100,
            ..Default::default()
        })
        .unwrap();

        let files = fs::list_files(u32::MAX, 2);
        assert_eq!(
            files.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
            vec!["f4", "f3"]
        );

        let files = fs::list_files(files.last().unwrap().id, 10);
        assert_eq!(
            files.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
            vec!["f2", "f1"]
        );
    }

    #[test]
    fn test_list_files_prev_zero() {
        fs::add_file(FileMetadata {
            name: "f1".to_string(),
            size: 100,
            ..Default::default()
        })
        .unwrap();

        // ids start at 1 and the range excludes `prev`, so these are empty
        // rather than a panic from `range(1..0)`
        assert!(fs::list_files(0, 10).is_empty());
        assert!(fs::list_files(1, 10).is_empty());
        assert_eq!(fs::list_files(2, 10).len(), 1);
    }

    #[test]
    fn test_save_load_large_metadata() {
        // the metadata is stored as `Chunk`, which is bounded to CHUNK_SIZE
        for i in 0..5000u32 {
            fs::add_file(FileMetadata {
                name: format!("some/reasonably/long/file/name/{}.bin", i),
                content_type: "application/octet-stream".to_string(),
                size: 100,
                ..Default::default()
            })
            .unwrap();
        }

        fs::save();
        assert_eq!(fs::with(|r| r.files.len()), 5000);

        // a later, smaller save must not leave trailing chunks behind for
        // load() to concatenate
        for i in 1..4900u32 {
            fs::delete_file(i).unwrap();
        }
        fs::save();

        fs::load();
        assert_eq!(fs::with(|r| r.files.len()), 101);
        assert_eq!(
            fs::get_file(5000).unwrap().name,
            "some/reasonably/long/file/name/4999.bin"
        );
    }

    #[test]
    fn test_update_chunk_limits() {
        fs::set_max_file_size(1024);
        let id = fs::add_file(FileMetadata {
            name: "f1".to_string(),
            ..Default::default()
        })
        .unwrap();

        assert_eq!(fs::update_chunk(id, 0, 1, vec![0u8; 512]).unwrap(), 512);
        assert_eq!(fs::update_chunk(id, 1, 1, vec![0u8; 512]).unwrap(), 1024);

        // rewriting a chunk of the same size keeps the file at the limit
        assert_eq!(fs::update_chunk(id, 1, 1, vec![1u8; 512]).unwrap(), 1024);
        assert_eq!(fs::get_file(id).unwrap().chunks, 2);

        // a chunk that would exceed the limit is rejected and must leave
        // `filled` untouched, the caller does not trap
        assert!(fs::update_chunk(id, 2, 1, vec![0u8; 512]).is_err());
        let file = fs::get_file(id).unwrap();
        assert_eq!(file.filled, 1024);
        assert_eq!(file.size, 1024);
        assert_eq!(file.chunks, 2);
        assert_eq!(fs::get_full_chunks(id).unwrap().len(), 1024);
    }

    #[test]
    fn test_milliseconds_constant() {
        // 1_000_000 ns per millisecond, matching every other ic-oss crate
        use crate::types::MILLISECONDS;
        assert_eq!(
            1_700_000_000_123_000_000u64 / MILLISECONDS,
            1_700_000_000_123
        );
    }
}
