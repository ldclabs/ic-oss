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
    fn test_chunk_index_bound_and_delete() {
        let id = fs::add_file(FileMetadata::default()).unwrap();
        assert!(fs::update_chunk(id, u32::MAX, 1, vec![1]).is_err());
        fs::update_chunk(id, u32::MAX - 1, 1, vec![1]).unwrap();
        assert_eq!(fs::get_file(id).unwrap().chunks, u32::MAX);
        // deletion scans the stored chunks instead of every index up to `chunks`
        assert!(fs::delete_file(id).unwrap());
        assert_eq!(fs::total_chunks(), 0);
    }

    #[test]
    fn test_file_id_reads_legacy_cbor_keys() {
        use ic_stable_structures::{storable::Bound, Storable, VectorMemory};
        use std::borrow::Cow;

        #[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
        struct LegacyFileId(u32, u32);
        impl Storable for LegacyFileId {
            const BOUND: Bound = Bound::Bounded {
                max_size: 11,
                is_fixed_size: false,
            };
            fn to_bytes(&self) -> Cow<'_, [u8]> {
                let mut buf = vec![];
                cbor2::to_writer(self, &mut buf).unwrap();
                Cow::Owned(buf)
            }
            fn into_bytes(self) -> Vec<u8> {
                self.to_bytes().into_owned()
            }
            fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
                cbor2::from_reader(&bytes[..]).unwrap()
            }
        }

        let key = FileId(u32::MAX, 7);
        assert_eq!(key.to_bytes().len(), 9);
        assert_eq!(FileId::from_bytes(key.to_bytes()), key);

        let memory = VectorMemory::default();
        let mut legacy = StableBTreeMap::<LegacyFileId, Chunk, _>::init(memory.clone());
        for i in 0..300u32 {
            legacy.insert(LegacyFileId(1, i), Chunk(vec![i as u8]));
        }
        drop(legacy);

        let mut m = StableBTreeMap::<FileId, Chunk, _>::init(memory);
        assert_eq!(m.get(&FileId(1, 7)).map(|c| c.0), Some(vec![7]));
        assert!(m.insert(FileId(1, 7), Chunk(vec![9])).is_some());
        assert_eq!(m.len(), 300);
        let keys: Vec<FileId> = m.keys_range(FileId(1, 0)..=FileId(1, u32::MAX)).collect();
        assert_eq!(keys, (0..300).map(|i| FileId(1, i)).collect::<Vec<_>>());
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
    #[test]
    fn inline_create_rejects_invalid_content_before_allocating_a_file() {
        use ic_oss_types::file::CreateFileInput;
        let input = CreateFileInput {
            name: "file.bin".into(),
            content_type: "application/octet-stream".into(),
            size: Some(3),
            content: Some(vec![1, 2].into()),
            ..Default::default()
        };
        assert!(fs::create_file(input.clone(), 0).is_err());
        assert_eq!(fs::with(|s| s.file_id), 1);
        assert!(fs::with(|s| s.files.is_empty()));
        assert_eq!(fs::total_chunks(), 0);
        fs::set_max_file_size(1);
        assert!(fs::create_file(
            CreateFileInput {
                size: None,
                ..input
            },
            0
        )
        .is_err());
        assert_eq!(fs::with(|s| s.file_id), 1);
    }
}
