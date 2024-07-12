use candid::Principal;
use ic_oss_types::permission::{
    Operation, Permission, PermissionChecker, PermissionCheckerAny, Policies, Resource,
};

use crate::store::fs;

pub fn check_bucket_read(ps: &Policies, bucket: &Principal) -> bool {
    ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::Other("Info".to_string())),
        },
        bucket.to_string(),
    )
}

pub fn check_folder_list(ps: &Policies, bucket: &Principal, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::List,
            constraint: Some(Resource::Folder),
        },
        bucket.to_string(),
    ) {
        let ancestors = fs::get_ancestors(parent);
        if !ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::List,
                constraint: None,
            },
            &ancestors,
        ) {
            return false;
        }
    }
    true
}

pub fn check_folder_read(ps: &Policies, bucket: &Principal, id: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::Folder),
        },
        bucket.to_string(),
    ) {
        let ancestors = fs::get_ancestors(id);
        if !ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Read,
                constraint: Some(Resource::Folder),
            },
            &ancestors,
        ) {
            return false;
        }
    }
    true
}

pub fn check_file_list(ps: &Policies, bucket: &Principal, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::List,
            constraint: Some(Resource::File),
        },
        bucket.to_string(),
    ) {
        let ancestors = fs::get_ancestors(parent);
        if !ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::List,
                constraint: Some(Resource::File),
            },
            &ancestors,
        ) {
            return false;
        }
    }
    true
}

pub fn check_file_read(ps: &Policies, bucket: &Principal, id: u32, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::File,
            operation: Operation::Read,
            constraint: None,
        },
        id.to_string(),
    ) && !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::File),
        },
        bucket.to_string(),
    ) {
        let ancestors = fs::get_ancestors(parent);
        if !ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Read,
                constraint: Some(Resource::File),
            },
            &ancestors,
        ) {
            return false;
        }
    }
    true
}

pub fn check_file_create(ps: &Policies, bucket: &Principal, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Write,
            constraint: Some(Resource::File),
        },
        bucket.to_string(),
    ) {
        let ancestors = fs::get_ancestors(parent);
        if !ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Write,
                constraint: Some(Resource::File),
            },
            &ancestors,
        ) {
            return false;
        }
    }
    true
}

pub fn check_file_delete(ps: &Policies, bucket: &Principal, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Delete,
            constraint: Some(Resource::File),
        },
        bucket.to_string(),
    ) {
        let ancestors = fs::get_ancestors(parent);
        if !ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Delete,
                constraint: Some(Resource::File),
            },
            &ancestors,
        ) {
            return false;
        }
    }
    true
}

pub fn check_file_update(ps: &Policies, bucket: &Principal, id: u32, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::File,
            operation: Operation::Write,
            constraint: None,
        },
        id.to_string(),
    ) {
        return check_file_create(ps, bucket, parent);
    }
    true
}

pub fn check_folder_create(ps: &Policies, bucket: &Principal, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Write,
            constraint: Some(Resource::Folder),
        },
        bucket.to_string(),
    ) {
        let ancestors = fs::get_ancestors(parent);
        if !ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Write,
                constraint: Some(Resource::Folder),
            },
            &ancestors,
        ) {
            return false;
        }
    }
    true
}

pub fn check_folder_delete(ps: &Policies, bucket: &Principal, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Delete,
            constraint: Some(Resource::Folder),
        },
        bucket.to_string(),
    ) {
        let ancestors = fs::get_ancestors(parent);
        if !ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Delete,
                constraint: Some(Resource::Folder),
            },
            &ancestors,
        ) {
            return false;
        }
    }
    true
}

pub fn check_folder_update(ps: &Policies, bucket: &Principal, id: u32, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Folder,
            operation: Operation::Write,
            constraint: None,
        },
        id.to_string(),
    ) {
        return check_folder_create(ps, bucket, parent);
    }
    true
}
