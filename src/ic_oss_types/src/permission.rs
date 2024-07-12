use std::collections::BTreeSet;
use std::fmt;
use std::ops::Deref;

/// Validate name of resource, operation, constraint, resource path, etc.
/// Valid characters: A-Z, a-z, 0-9, _, -
pub fn validate_name(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("empty string".to_string());
    }

    for c in s.chars() {
        if !matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-') {
            return Err(format!("invalid character: {}", c));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Resource {
    #[default]
    All, // "*" means all resources
    File,
    Folder,
    Bucket,
    Cluster,
    Other(String),
}

impl Resource {
    pub fn check(&self, value: &Resource) -> bool {
        match self {
            Self::All => true,
            other => value == other,
        }
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "*"),
            Self::File => write!(f, "File"),
            Self::Folder => write!(f, "Folder"),
            Self::Bucket => write!(f, "Bucket"),
            Self::Cluster => write!(f, "Cluster"),
            Self::Other(ref s) => write!(f, "{}", s),
        }
    }
}

impl TryFrom<&str> for Resource {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "*" => Ok(Self::All),
            "File" => Ok(Self::File),
            "Folder" => Ok(Self::Folder),
            "Bucket" => Ok(Self::Bucket),
            "Cluster" => Ok(Self::Cluster),
            _ => match validate_name(value) {
                Ok(_) => Ok(Self::Other(value.to_string())),
                Err(err) => Err(format!("invalid resource: {}, {}", value, err)),
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    #[default]
    All, // "*" means all operations
    List,
    Read,
    Write,
    Delete,
    Other(String),
}

impl Operation {
    pub fn check(&self, value: &Operation) -> bool {
        match self {
            Self::All => true,
            other => value == other,
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "*"),
            Self::List => write!(f, "List"),
            Self::Read => write!(f, "Read"),
            Self::Write => write!(f, "Write"),
            Self::Delete => write!(f, "Delete"),
            Self::Other(ref s) => write!(f, "{}", s),
        }
    }
}

impl TryFrom<&str> for Operation {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "*" => Ok(Self::All),
            "List" => Ok(Self::List),
            "Read" => Ok(Self::Read),
            "Write" => Ok(Self::Write),
            "Delete" => Ok(Self::Delete),
            _ => match validate_name(value) {
                Ok(_) => Ok(Self::Other(value.to_string())),
                Err(err) => Err(format!("invalid operation: {}, {}", value, err)),
            },
        }
    }
}

/// Permission string format: Resource.Operation[.Constraint]
/// e.g. File.Read Folder.Write Bucket.Read Bucket.Read.Info
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Permission {
    pub resource: Resource,
    pub operation: Operation,
    pub constraint: Option<Resource>, // ignore for now, reserved for future use
}

impl Permission {
    pub fn is_all(&self) -> bool {
        self.resource == Resource::All
            && self.operation == Operation::All
            && self.constraint.is_none()
    }

    pub fn check(&self, value: &Permission) -> bool {
        self.resource.check(&value.resource)
            && self.operation.check(&value.operation)
            && self.check_constraint(&value.constraint)
    }

    pub fn check_constraint(&self, value: &Option<Resource>) -> bool {
        match self.constraint {
            None | Some(Resource::All) => true,
            Some(ref c) => value.as_ref().map_or(false, |v| c == v),
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.constraint {
            Some(ref c) if c != &Resource::All => {
                write!(f, "{}.{}.{}", self.resource, self.operation, c)
            }
            _ => {
                if self.is_all() {
                    write!(f, "*")
                } else {
                    write!(f, "{}.{}", self.resource, self.operation)
                }
            }
        }
    }
}

impl TryFrom<&str> for Permission {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value == "*" {
            return Ok(Self {
                resource: Resource::All,
                operation: Operation::All,
                constraint: None,
            });
        }

        let mut parts = value.split('.');
        let resource = match parts.next() {
            Some(v) => Resource::try_from(v)?,
            _ => return Err(format!("invalid permission format {}", value)),
        };

        let operation = match parts.next() {
            Some(v) => Operation::try_from(v)?,
            _ => return Err(format!("invalid permission format {}", value)),
        };

        let constraint = match parts.next() {
            Some(v) => {
                Some(Resource::try_from(v).map_err(|err| format!("invalid constraint: {}", err))?)
            }
            None => None,
        };

        if parts.next().is_some() {
            return Err(format!("invalid permission format {}", value));
        }

        Ok(Self {
            resource,
            operation,
            constraint,
        })
    }
}

pub type ResourcePath = String;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Resources(pub BTreeSet<ResourcePath>);

impl Resources {
    pub fn is_all(&self) -> bool {
        self.0.is_empty() || self.0.contains("*")
    }

    fn check<T>(&self, value: T) -> bool
    where
        T: AsRef<str>,
    {
        self.is_all() || self.0.contains(value.as_ref())
    }
}

impl Deref for Resources {
    type Target = BTreeSet<ResourcePath>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<BTreeSet<ResourcePath>> for Resources {
    fn as_ref(&self) -> &BTreeSet<ResourcePath> {
        &self.0
    }
}

impl fmt::Display for Resources {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.first() {
            None => Ok(()),
            Some(v) => {
                if !self.is_all() {
                    write!(f, "{}", v)?;
                    for r in self.0.iter().skip(1) {
                        write!(f, ",{}", r)?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl<const N: usize> From<[ResourcePath; N]> for Resources {
    fn from(val: [ResourcePath; N]) -> Self {
        Self(BTreeSet::from(val))
    }
}

impl TryFrom<&str> for Resources {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "" | "*" => Ok(Self::default()),
            _ => {
                let rs: BTreeSet<_> = value.split(',').map(|v| v.to_string()).collect();
                for r in rs.iter() {
                    validate_name(r)?;
                }
                Ok(Resources(rs))
            }
        }
    }
}

pub trait PermissionChecker<T> {
    fn has_permission(&self, permission: &Permission, resource_path: T) -> bool;
}

pub trait PermissionCheckerAny<T> {
    fn has_permission_any(&self, permission: &Permission, resources_path: &[T]) -> bool;
}

/// Policy string format: Permission:Resource1,Resource2,...
/// e.g. File.*:* File.Read:* Folder.Write:1,2 Bucket.Read:bucket1,bucket2
/// e.g. *.*:* *:*  *
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Policy {
    pub permission: Permission,
    pub resources: Resources,
}

impl<T> PermissionChecker<T> for Policy
where
    T: AsRef<str>,
{
    fn has_permission(&self, permission: &Permission, resource_path: T) -> bool {
        self.permission.check(permission) && self.resources.check(resource_path.as_ref())
    }
}

impl<T> PermissionCheckerAny<T> for Policy
where
    T: AsRef<str>,
{
    fn has_permission_any(&self, permission: &Permission, resources_path: &[T]) -> bool {
        self.permission.check(permission)
            && (self.resources.is_all() || resources_path.iter().any(|r| self.resources.check(r)))
    }
}

impl fmt::Display for Policy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.resources.is_all() {
            if self.permission.is_all() {
                write!(f, "*")
            } else {
                write!(f, "{}", self.permission)
            }
        } else {
            write!(f, "{}:{}", self.permission, self.resources)
        }
    }
}

impl TryFrom<&str> for Policy {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value == "*" {
            return Ok(Self::default());
        }

        let mut parts = value.split(':');
        let permission = match parts.next() {
            Some(v) => Permission::try_from(v)?,
            _ => return Err(format!("invalid policy format {}", value)),
        };

        let resources = match parts.next() {
            Some(v) => Resources::try_from(v)?,
            _ => Resources::default(),
        };

        if parts.next().is_some() {
            return Err(format!("invalid policy format {}", value));
        }

        Ok(Self {
            permission,
            resources,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Policies(pub BTreeSet<Policy>);

impl Policies {
    pub fn all() -> Self {
        Self(BTreeSet::from([Policy::default()]))
    }

    pub fn append(&mut self, policies: &mut Policies) {
        self.0.append(&mut policies.0);
    }

    pub fn remove(&mut self, policies: &Policies) {
        self.0.retain(|p| !policies.0.contains(p));
    }
}

impl Deref for Policies {
    type Target = BTreeSet<Policy>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<BTreeSet<Policy>> for Policies {
    fn as_ref(&self) -> &BTreeSet<Policy> {
        &self.0
    }
}

impl<T> PermissionChecker<T> for Policies
where
    T: AsRef<str>,
{
    fn has_permission(&self, permission: &Permission, resource_path: T) -> bool {
        self.0
            .iter()
            .any(|p| p.has_permission(permission, resource_path.as_ref()))
    }
}

impl<T> PermissionCheckerAny<T> for Policies
where
    T: AsRef<str>,
{
    fn has_permission_any(&self, permission: &Permission, resources_any: &[T]) -> bool {
        self.0
            .iter()
            .any(|p| p.has_permission_any(permission, resources_any))
    }
}

impl fmt::Display for Policies {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.first() {
            None => Ok(()),
            Some(v) => {
                write!(f, "{}", v)?;
                for r in self.0.iter().skip(1) {
                    write!(f, " {}", r)?;
                }
                Ok(())
            }
        }
    }
}

impl<const N: usize> From<[Policy; N]> for Policies {
    fn from(val: [Policy; N]) -> Self {
        Self(BTreeSet::from(val))
    }
}

impl TryFrom<&str> for Policies {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Ok(Self::default());
        }

        let policies = value
            .split(' ')
            .map(Policy::try_from)
            .collect::<Result<BTreeSet<_>, _>>()?;
        Ok(Policies(policies))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        assert!(validate_name("").is_err());
        assert!(validate_name("*").is_err());
        assert!(validate_name(" ").is_err());
        assert!(validate_name(".").is_err());
        assert!(validate_name(",").is_err());
        assert!(validate_name(".Info").is_err());
        assert!(validate_name("Info").is_ok());
        assert!(validate_name("123").is_ok());
        assert!(validate_name("Level_1").is_ok());
        assert!(validate_name("mmrxu-fqaaa-aaaap-ahhna-cai").is_ok());
    }

    #[test]
    fn test_permission() {
        for (s, p) in [
            (
                "Bucket.Read.Info",
                Permission {
                    resource: Resource::Bucket,
                    operation: Operation::Read,
                    constraint: Some(Resource::Other("Info".to_string())),
                },
            ),
            (
                "Bucket.Read.File",
                Permission {
                    resource: Resource::Bucket,
                    operation: Operation::Read,
                    constraint: Some(Resource::File),
                },
            ),
            (
                "SomeResource.some_operation",
                Permission {
                    resource: Resource::Other("SomeResource".to_string()),
                    operation: Operation::Other("some_operation".to_string()),
                    constraint: None,
                },
            ),
            (
                "File.Read",
                Permission {
                    resource: Resource::File,
                    operation: Operation::Read,
                    constraint: None,
                },
            ),
            (
                "File.*",
                Permission {
                    resource: Resource::File,
                    operation: Operation::All,
                    constraint: None,
                },
            ),
            (
                "*.Read",
                Permission {
                    resource: Resource::All,
                    operation: Operation::Read,
                    constraint: None,
                },
            ),
            (
                "*",
                Permission {
                    resource: Resource::All,
                    operation: Operation::All,
                    constraint: None,
                },
            ),
        ] {
            assert_eq!(p.to_string(), s, "Permission({})", s);
            assert_eq!(Permission::try_from(s).unwrap(), p);
        }

        assert!(Permission::try_from(".File").is_err());
        assert!(Permission::try_from("File").is_err());
        assert!(Permission::try_from("File.").is_err());
        assert!(Permission::try_from("File.Read.Info.Info").is_err());

        assert!(Permission::default().check(&Permission::default()));
        assert!(Permission::default().check(&Permission {
            resource: Resource::File,
            operation: Operation::Read,
            constraint: None,
        }));
        assert!(Permission::default().check(&Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::File),
        }));
        assert!(Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: None,
        }
        .check(&Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::Other("Info".to_string())),
        }));

        assert!(!Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::Other("Info".to_string())),
        }
        .check(&Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: Some(Resource::File),
        }));
        assert!(!Permission {
            resource: Resource::Bucket,
            operation: Operation::Write,
            constraint: None,
        }
        .check(&Permission {
            resource: Resource::Bucket,
            operation: Operation::Read,
            constraint: None,
        }));
        assert!(!Permission {
            resource: Resource::Folder,
            operation: Operation::Write,
            constraint: None,
        }
        .check(&Permission {
            resource: Resource::File,
            operation: Operation::Write,
            constraint: None,
        }));
    }

    #[test]
    fn test_resources() {
        let rs = Resources::default();
        assert_eq!(rs.to_string(), "");
        assert_eq!(Resources::try_from("").unwrap(), rs);
        assert!(rs.check(""));
        assert!(rs.check("123"));
        assert!(rs.check("abc"));

        let rs = Resources::try_from("*").unwrap();
        assert!(rs.check(""));
        assert!(rs.check("123"));
        assert!(rs.check("abc"));
        assert_eq!(rs.to_string(), "");

        let rs = Resources::from(["1".to_string()]);
        assert_eq!(rs.to_string(), "1");
        assert_eq!(Resources::try_from("1").unwrap(), rs);
        assert!(rs.check("1"));
        assert!(!rs.check("2"));
        assert!(!rs.check(""));
        assert!(!rs.check("12"));
        assert!(!rs.check("a"));

        let rs = Resources::from(["1".to_string(), "2".to_string(), "3".to_string()]);
        assert_eq!(rs.to_string(), "1,2,3");
        assert_eq!(Resources::try_from("1,2,3").unwrap(), rs);
        assert!(rs.check("1"));
        assert!(rs.check("2"));
        assert!(!rs.check(""));
        assert!(!rs.check("12"));
        assert!(!rs.check("a"));

        assert!(Resources::try_from("1, 2").is_err());
        assert!(Resources::try_from("1,2 ").is_err());
        assert!(Resources::try_from("1,2.3").is_err());
    }

    #[test]
    fn test_policy() {
        let po = Policy::default();
        assert_eq!(po.to_string(), "*");
        assert_eq!(Policy::try_from("*").unwrap(), po);
        assert_eq!(Policy::try_from("*:*").unwrap(), po);
        assert_eq!(Policy::try_from("*.*:*").unwrap(), po);
        assert!(po.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Read,
                constraint: None,
            },
            ""
        ));
        assert!(po.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Write,
                constraint: None,
            },
            "1"
        ));

        let po = Policy {
            permission: Permission {
                resource: Resource::File,
                operation: Operation::All,
                constraint: None,
            },
            resources: Resources::from(["123".to_string()]),
        };
        assert_eq!(po.to_string(), "File.*:123");
        assert_eq!(Policy::try_from("File.*:123").unwrap(), po);
        assert!(po.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Read,
                constraint: None,
            },
            "123"
        ));
        assert!(po.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Write,
                constraint: None,
            },
            "123"
        ));
        assert!(!po.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Read,
                constraint: None,
            },
            "1"
        ));
        assert!(!po.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Write,
                constraint: None,
            },
            "1"
        ));
        assert!(!po.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Write,
                constraint: None,
            },
            "123"
        ));
        assert!(!po.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Write,
                constraint: None,
            },
            ""
        ));
    }

    #[test]
    fn test_policies() {
        let ps = Policies::default();
        assert_eq!(ps.to_string(), "");
        assert!(!ps.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Read,
                constraint: None,
            },
            ""
        ));

        let ps = Policies::all();

        assert_eq!(Policies::try_from("*").unwrap(), ps);
        assert_eq!(Policies::try_from("*:*").unwrap(), ps);
        assert_eq!(Policies::try_from("*.*:*").unwrap(), ps);
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Read,
                constraint: None,
            },
            ""
        ));
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Read,
                constraint: None,
            },
            "123"
        ));
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::Bucket,
                operation: Operation::Write,
                constraint: Some(Resource::Folder),
            },
            "bucket1"
        ));

        let ps = Policies::from([
            Policy {
                permission: Permission {
                    resource: Resource::Bucket,
                    operation: Operation::Read,
                    constraint: Some(Resource::All),
                },
                resources: Resources::from([]),
            },
            Policy {
                permission: Permission {
                    resource: Resource::Folder,
                    operation: Operation::Read,
                    constraint: None,
                },
                resources: Resources::default(),
            },
            Policy {
                permission: Permission {
                    resource: Resource::Folder,
                    operation: Operation::All,
                    constraint: None,
                },
                resources: Resources::from(["2".to_string(), "3".to_string(), "5".to_string()]),
            },
            Policy {
                permission: Permission {
                    resource: Resource::File,
                    operation: Operation::All,
                    constraint: None,
                },
                resources: Resources::from(["1".to_string()]),
            },
        ]);

        println!("{}", ps);
        let scope = "File.*:1 Folder.*:2,3,5 Folder.Read Bucket.Read";
        assert_eq!(ps.to_string(), scope);
        assert_eq!(Policies::try_from(scope).unwrap().to_string(), scope);

        // File.*:1
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Delete,
                constraint: None,
            },
            "1"
        ));

        // File.*:1
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Read,
                constraint: Some(Resource::Other("Info".to_string())),
            },
            "1"
        ));

        // File.*:1
        assert!(!ps.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::Read,
                constraint: Some(Resource::Other("Info".to_string())),
            },
            "2"
        ));

        // File.*:1
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::File,
                operation: Operation::All,
                constraint: None,
            },
            "1"
        ));

        // Folder.*:2,3,5
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Delete,
                constraint: Some(Resource::File),
            },
            "2"
        ));

        // Folder.*:2,3,5
        assert!(!ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Delete,
                constraint: Some(Resource::File),
            },
            "4"
        ));

        // Folder.*:2,3,5
        assert!(ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Delete,
                constraint: Some(Resource::File),
            },
            &["4", "5"]
        ));
        assert!(ps.has_permission_any(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Delete,
                constraint: Some(Resource::File),
            },
            &[4.to_string(), 5.to_string()]
        ));

        // Folder.Read
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Read,
                constraint: Some(Resource::Other("Info".to_string())),
            },
            "1"
        ));

        // Folder.Read
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::Folder,
                operation: Operation::Read,
                constraint: Some(Resource::File),
            },
            "6"
        ));

        // Bucket.Read
        assert!(ps.has_permission(
            &Permission {
                resource: Resource::Bucket,
                operation: Operation::Read,
                constraint: Some(Resource::Folder),
            },
            "1"
        ));

        // Bucket.Read
        assert!(!ps.has_permission(
            &Permission {
                resource: Resource::Bucket,
                operation: Operation::Write,
                constraint: Some(Resource::Folder),
            },
            "1"
        ));
    }
}