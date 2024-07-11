use candid::Principal;
use ic_oss_types::permission::{Operation, Permission, PermissionChecker, Policies, Resource};

use crate::store::fs;

pub fn check_bucket_read(ps: &Policies, bucket: &Principal) -> bool {
    ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::Other("Info".to_string())),
        },
        bucket.to_string().as_str(),
    )
}

pub fn check_folder_list(ps: &Policies, bucket: &Principal, parent: u32) -> bool {
    if !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::List,
            constraint: Some(Resource::Folder),
        },
        bucket.to_string().as_str(),
    ) {
        let ancestors: Vec<String> = fs::get_ancestors(parent)
            .into_iter()
            .map(|f| f.id.to_string())
            .collect();
        let rs: Vec<&str> = ancestors.iter().map(|id| id.as_str()).collect();
        if !ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::List,
                constraint: None,
            },
            rs.as_slice(),
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
        bucket.to_string().as_str(),
    ) {
        let ancestors: Vec<String> = fs::get_ancestors(id)
            .into_iter()
            .map(|f| f.id.to_string())
            .collect();
        let rs: Vec<&str> = ancestors.iter().map(|id| id.as_str()).collect();
        if !ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Read,
                constraint: Some(Resource::Folder),
            },
            rs.as_slice(),
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
        bucket.to_string().as_str(),
    ) {
        let ancestors: Vec<String> = fs::get_ancestors(parent)
            .into_iter()
            .map(|f| f.id.to_string())
            .collect();
        let rs: Vec<&str> = ancestors.iter().map(|id| id.as_str()).collect();
        if !ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::List,
                constraint: Some(Resource::File),
            },
            rs.as_slice(),
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
        id.to_string().as_str(),
    ) && !ps.has_permission(
        &Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::File),
        },
        bucket.to_string().as_str(),
    ) {
        let ancestors: Vec<String> = fs::get_ancestors(parent)
            .into_iter()
            .map(|f| f.id.to_string())
            .collect();
        let rs: Vec<&str> = ancestors.iter().map(|id| id.as_str()).collect();
        if !ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Read,
                constraint: Some(Resource::File),
            },
            rs.as_slice(),
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
        bucket.to_string().as_str(),
    ) {
        let ancestors: Vec<String> = fs::get_ancestors(parent)
            .into_iter()
            .map(|f| f.id.to_string())
            .collect();
        let rs: Vec<&str> = ancestors.iter().map(|id| id.as_str()).collect();
        if !ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Write,
                constraint: Some(Resource::File),
            },
            rs.as_slice(),
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
        bucket.to_string().as_str(),
    ) {
        let ancestors: Vec<String> = fs::get_ancestors(parent)
            .into_iter()
            .map(|f| f.id.to_string())
            .collect();
        let rs: Vec<&str> = ancestors.iter().map(|id| id.as_str()).collect();
        if !ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Delete,
                constraint: Some(Resource::File),
            },
            rs.as_slice(),
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
        id.to_string().as_str(),
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
        bucket.to_string().as_str(),
    ) {
        let ancestors: Vec<String> = fs::get_ancestors(parent)
            .into_iter()
            .map(|f| f.id.to_string())
            .collect();
        let rs: Vec<&str> = ancestors.iter().map(|id| id.as_str()).collect();
        if !ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Write,
                constraint: Some(Resource::Folder),
            },
            rs.as_slice(),
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
        bucket.to_string().as_str(),
    ) {
        let ancestors: Vec<String> = fs::get_ancestors(parent)
            .into_iter()
            .map(|f| f.id.to_string())
            .collect();
        let rs: Vec<&str> = ancestors.iter().map(|id| id.as_str()).collect();
        if !ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Delete,
                constraint: Some(Resource::Folder),
            },
            rs.as_slice(),
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
        id.to_string().as_str(),
    ) {
        return check_folder_create(ps, bucket, parent);
    }
    true
}
