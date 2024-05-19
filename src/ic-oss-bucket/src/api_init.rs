use ic_oss_types::file::MAX_FILE_SIZE;

use crate::store;

#[ic_cdk::init]
fn init() {
    store::state::with_mut(|b| {
        b.name = "default".to_string();
        b.max_file_size = MAX_FILE_SIZE;
        b.max_dir_depth = 10;
        b.max_children = 1000;
    });

    store::state::save();
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    store::state::save();
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    store::state::load();
}
