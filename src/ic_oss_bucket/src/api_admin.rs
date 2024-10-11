use candid::Principal;
use ic_oss_types::bucket::UpdateBucketInput;
use std::collections::BTreeSet;

use crate::{is_controller, store, validate_principals};

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_admin_set_managers(args.clone())?;
    store::state::with_mut(|r| {
        r.managers = args;
    });
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_managers(mut args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers.append(&mut args);
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_remove_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers.retain(|p| !args.contains(p));
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_auditors(mut args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.auditors.append(&mut args);
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_remove_auditors(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.auditors.retain(|p| !args.contains(p));
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_auditors(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.auditors = args;
    });
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_update_bucket(args: UpdateBucketInput) -> Result<(), String> {
    args.validate()?;
    store::state::with_mut(|s| {
        if let Some(name) = args.name {
            s.name = name;
        }
        if let Some(max_file_size) = args.max_file_size {
            s.max_file_size = max_file_size;
        }
        if let Some(max_folder_depth) = args.max_folder_depth {
            s.max_folder_depth = max_folder_depth;
        }
        if let Some(max_children) = args.max_children {
            s.max_children = max_children;
        }
        if let Some(max_custom_data_size) = args.max_custom_data_size {
            s.max_custom_data_size = max_custom_data_size;
        }
        if let Some(enable_hash_index) = args.enable_hash_index {
            s.enable_hash_index = enable_hash_index;
        }
        if let Some(status) = args.status {
            s.status = status;
        }
        if let Some(visibility) = args.visibility {
            s.visibility = visibility;
        }
        if let Some(trusted_ecdsa_pub_keys) = args.trusted_ecdsa_pub_keys {
            s.trusted_ecdsa_pub_keys = trusted_ecdsa_pub_keys;
        }
        if let Some(trusted_eddsa_pub_keys) = args.trusted_eddsa_pub_keys {
            s.trusted_eddsa_pub_keys = trusted_eddsa_pub_keys;
        }
    });
    Ok(())
}

// ----- Use validate2_xxxxxx instead of validate_xxxxxx -----

#[ic_cdk::update]
fn validate_admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    Ok(())
}

#[ic_cdk::update]
fn validate2_admin_set_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_set_auditors(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    Ok(())
}

#[ic_cdk::update]
fn validate2_admin_set_auditors(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_update_bucket(args: UpdateBucketInput) -> Result<(), String> {
    args.validate()
}

#[ic_cdk::update]
fn validate2_admin_update_bucket(args: UpdateBucketInput) -> Result<String, String> {
    args.validate()?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_add_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_remove_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_add_auditors(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_remove_auditors(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}
