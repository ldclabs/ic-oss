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
}
