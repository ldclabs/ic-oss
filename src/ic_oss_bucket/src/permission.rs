use candid::Principal;
use ic_oss_types::{
    cose::{Token, BUCKET_TOKEN_AAD},
    permission::{
        Operation, Permission, PermissionChecker, PermissionCheckerAny, Policies, Resource,
    },
};
use once_cell::sync::Lazy;
use serde_bytes::ByteBuf;

use crate::{store, SECONDS};

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum Role {
    User,
    Auditor,
    Manager,
}

#[derive(Clone, Debug)]
enum Access {
    Full,
    ReadOnly,
    Scoped {
        policies: Policies,
        bucket_path: String,
    },
}

/// The authorization result used by the bucket's storage interface.
///
/// Common manager, auditor, and public-read requests use allocation-free access
/// variants. Token policies are only parsed and retained for scoped requests.
#[derive(Clone, Debug)]
pub struct Context {
    access: Access,
    pub role: Role,
}

impl Context {
    fn full(role: Role) -> Self {
        Self {
            access: Access::Full,
            role,
        }
    }

    fn read_only(role: Role) -> Self {
        Self {
            access: Access::ReadOnly,
            role,
        }
    }

    fn scoped(role: Role, policies: Policies, bucket: &Principal) -> Self {
        Self {
            access: Access::Scoped {
                policies,
                bucket_path: bucket.to_string(),
            },
            role,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_full(role: Role) -> Self {
        Self::full(role)
    }

    fn has_permission_any(&self, permission: &Permission, resource_paths: &[String]) -> bool {
        match &self.access {
            Access::Full => true,
            Access::ReadOnly => matches!(permission.operation, Operation::Read | Operation::List),
            Access::Scoped { policies, .. } => {
                policies.has_permission_any(permission, resource_paths)
            }
        }
    }

    fn has_bucket_permission(&self, permission: &Permission) -> bool {
        match &self.access {
            Access::Full => true,
            Access::ReadOnly => matches!(permission.operation, Operation::Read | Operation::List),
            Access::Scoped {
                policies,
                bucket_path,
            } => policies.has_permission(permission, bucket_path),
        }
    }

    fn has_id_permission(&self, permission: &Permission, id: u32) -> bool {
        match &self.access {
            Access::Full => true,
            Access::ReadOnly => matches!(permission.operation, Operation::Read | Operation::List),
            Access::Scoped { policies, .. } => policies.has_permission(permission, id.to_string()),
        }
    }
}

const BUCKET_LIST_FOLDER: Permission = Permission {
    resource: Resource::Bucket,
    operation: Operation::List,
    constraint: Some(Resource::Folder),
};
const BUCKET_READ_FOLDER: Permission = Permission {
    resource: Resource::Bucket,
    operation: Operation::Read,
    constraint: Some(Resource::Folder),
};
const BUCKET_LIST_FILE: Permission = Permission {
    resource: Resource::Bucket,
    operation: Operation::List,
    constraint: Some(Resource::File),
};
const BUCKET_READ_FILE: Permission = Permission {
    resource: Resource::Bucket,
    operation: Operation::Read,
    constraint: Some(Resource::File),
};
const BUCKET_WRITE_FILE: Permission = Permission {
    resource: Resource::Bucket,
    operation: Operation::Write,
    constraint: Some(Resource::File),
};
const BUCKET_DELETE_FILE: Permission = Permission {
    resource: Resource::Bucket,
    operation: Operation::Delete,
    constraint: Some(Resource::File),
};
const BUCKET_WRITE_FOLDER: Permission = Permission {
    resource: Resource::Bucket,
    operation: Operation::Write,
    constraint: Some(Resource::Folder),
};
const BUCKET_DELETE_FOLDER: Permission = Permission {
    resource: Resource::Bucket,
    operation: Operation::Delete,
    constraint: Some(Resource::Folder),
};
const FOLDER_LIST_FOLDER: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::List,
    constraint: Some(Resource::Folder),
};
const FOLDER_READ_FOLDER: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::Read,
    constraint: Some(Resource::Folder),
};
const FOLDER_LIST_FILE: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::List,
    constraint: Some(Resource::File),
};
const FOLDER_READ_FILE: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::Read,
    constraint: Some(Resource::File),
};
const FOLDER_WRITE_FILE: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::Write,
    constraint: Some(Resource::File),
};
const FOLDER_DELETE_FILE: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::Delete,
    constraint: Some(Resource::File),
};
const FOLDER_WRITE_FOLDER: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::Write,
    constraint: Some(Resource::Folder),
};
const FOLDER_DELETE_FOLDER: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::Delete,
    constraint: Some(Resource::Folder),
};
const FILE_READ: Permission = Permission {
    resource: Resource::File,
    operation: Operation::Read,
    constraint: None,
};
const FILE_WRITE: Permission = Permission {
    resource: Resource::File,
    operation: Operation::Write,
    constraint: None,
};
const FOLDER_WRITE: Permission = Permission {
    resource: Resource::Folder,
    operation: Operation::Write,
    constraint: None,
};

static BUCKET_READ_INFO: Lazy<Permission> = Lazy::new(|| Permission {
    resource: Resource::Bucket,
    operation: Operation::Read,
    constraint: Some(Resource::Other("Info".to_string())),
});

pub fn authorize_read(access_token: Option<ByteBuf>) -> Result<Context, (u16, String)> {
    let now_sec = ic_cdk::api::time() / SECONDS;
    let caller = ic_cdk::api::msg_caller();
    let canister = ic_cdk::api::canister_self();
    store::state::with(|bucket| {
        let role = role(bucket, &caller);

        if bucket.status < 0 {
            if role >= Role::Auditor {
                return Ok(Context::read_only(role));
            }
            return Err((403, "bucket is archived".to_string()));
        }

        if bucket.visibility > 0 || role >= Role::Auditor {
            return Ok(Context::read_only(role));
        }

        let policies = token_policies(bucket, &canister, access_token, now_sec)?;
        Ok(Context::scoped(role, policies, &canister))
    })
}

pub fn authorize_write(
    access_token: Option<ByteBuf>,
    now_sec: u64,
) -> Result<Context, (u16, String)> {
    let caller = ic_cdk::api::msg_caller();
    let canister = ic_cdk::api::canister_self();
    store::state::with(|bucket| {
        if bucket.status != 0 {
            return Err((403, "bucket is not writable".to_string()));
        }

        let role = role(bucket, &caller);
        if role == Role::Manager {
            return Ok(Context::full(role));
        }

        let policies = token_policies(bucket, &canister, access_token, now_sec)?;
        Ok(Context::scoped(role, policies, &canister))
    })
}

fn role(bucket: &store::Bucket, caller: &Principal) -> Role {
    if bucket.managers.contains(caller) {
        Role::Manager
    } else if bucket.auditors.contains(caller) {
        Role::Auditor
    } else {
        Role::User
    }
}

fn token_policies(
    bucket: &store::Bucket,
    canister: &Principal,
    access_token: Option<ByteBuf>,
    now_sec: u64,
) -> Result<Policies, (u16, String)> {
    if let Some(token) = access_token {
        let token = Token::from_sign1(
            &token,
            &bucket.trusted_ecdsa_pub_keys,
            &bucket.trusted_eddsa_pub_keys,
            BUCKET_TOKEN_AAD,
            now_sec as i64,
        )
        .map_err(|err| (401, err))?;

        if &token.audience == canister {
            return Policies::try_from(token.policies.as_str()).map_err(|err| (403, err));
        }
    }

    Err((401, "Unauthorized".to_string()))
}

fn has_ancestor_permission(ctx: &Context, permission: &Permission, parent: u32) -> bool {
    // A policy with an unrestricted resource set does not need the ancestor
    // path to be materialized as decimal strings.
    if ctx.has_permission_any(permission, &[]) {
        return true;
    }

    let ancestors = store::fs::get_ancestors(parent);
    ctx.has_permission_any(permission, &ancestors)
}

pub fn check_bucket_read(ctx: &Context) -> bool {
    ctx.has_bucket_permission(&BUCKET_READ_INFO)
}

pub fn check_folder_list(ctx: &Context, parent: u32) -> bool {
    ctx.has_bucket_permission(&BUCKET_LIST_FOLDER)
        || has_ancestor_permission(ctx, &FOLDER_LIST_FOLDER, parent)
}

pub fn check_folder_read(ctx: &Context, id: u32) -> bool {
    ctx.has_bucket_permission(&BUCKET_READ_FOLDER)
        || has_ancestor_permission(ctx, &FOLDER_READ_FOLDER, id)
}

pub fn check_file_list(ctx: &Context, parent: u32) -> bool {
    ctx.has_bucket_permission(&BUCKET_LIST_FILE)
        || has_ancestor_permission(ctx, &FOLDER_LIST_FILE, parent)
}

pub fn check_file_read(ctx: &Context, id: u32, parent: u32) -> bool {
    ctx.has_id_permission(&FILE_READ, id)
        || ctx.has_bucket_permission(&BUCKET_READ_FILE)
        || has_ancestor_permission(ctx, &FOLDER_READ_FILE, parent)
}

pub fn check_file_create(ctx: &Context, parent: u32) -> bool {
    ctx.has_bucket_permission(&BUCKET_WRITE_FILE)
        || has_ancestor_permission(ctx, &FOLDER_WRITE_FILE, parent)
}

pub fn check_file_delete(ctx: &Context, parent: u32) -> bool {
    ctx.has_bucket_permission(&BUCKET_DELETE_FILE)
        || has_ancestor_permission(ctx, &FOLDER_DELETE_FILE, parent)
}

pub fn check_file_update(ctx: &Context, id: u32, parent: u32) -> bool {
    ctx.has_id_permission(&FILE_WRITE, id) || check_file_create(ctx, parent)
}

pub fn check_folder_create(ctx: &Context, parent: u32) -> bool {
    ctx.has_bucket_permission(&BUCKET_WRITE_FOLDER)
        || has_ancestor_permission(ctx, &FOLDER_WRITE_FOLDER, parent)
}

pub fn check_folder_delete(ctx: &Context, parent: u32) -> bool {
    ctx.has_bucket_permission(&BUCKET_DELETE_FOLDER)
        || has_ancestor_permission(ctx, &FOLDER_DELETE_FOLDER, parent)
}

pub fn check_folder_update(ctx: &Context, id: u32, parent: u32) -> bool {
    ctx.has_id_permission(&FOLDER_WRITE, id) || check_folder_create(ctx, parent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_access_modes_keep_read_and_write_semantics() {
        let read_only = Context::read_only(Role::User);
        assert!(check_bucket_read(&read_only));
        assert!(check_file_read(&read_only, 1, 0));
        assert!(check_folder_list(&read_only, 0));
        assert!(!check_file_create(&read_only, 0));
        assert!(!check_folder_delete(&read_only, 0));

        let full = Context::full(Role::Manager);
        assert!(check_file_create(&full, 0));
        assert!(check_file_update(&full, 1, 0));
        assert!(check_folder_delete(&full, 0));
    }

    #[test]
    fn scoped_access_checks_bucket_ids_and_unrestricted_resources() {
        let bucket = Principal::from_text("aaaaa-aa").unwrap();
        let policies =
            Policies::try_from(format!("Bucket.Read.File:{} File.Write:7", bucket).as_str())
                .unwrap();
        let scoped = Context::scoped(Role::User, policies, &bucket);

        assert!(check_file_read(&scoped, 99, 0));
        assert!(check_file_update(&scoped, 7, 0));
        assert!(!check_file_update(&scoped, 8, 0));
        assert!(!check_folder_create(&scoped, 0));

        let unrestricted = Context::scoped(
            Role::User,
            Policies::try_from("Folder.Write.File").unwrap(),
            &bucket,
        );
        assert!(check_file_create(&unrestricted, 0));
    }
}
