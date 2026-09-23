use candid::{pretty::candid::value::pp_value, CandidType, IDLValue, Principal};
use ic_oss_types::{bucket::UpdateBucketInput, validate_principals};
use std::collections::BTreeSet;

use crate::{is_controller, store};

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers = args;
    });
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers.extend(args);
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_remove_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        for principal in args {
            r.managers.remove(&principal);
        }
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_auditors(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.auditors.extend(args);
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_remove_auditors(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        for principal in args {
            r.auditors.remove(&principal);
        }
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
    store::state::validate_update(&args)?;
    store::state::apply_update(args);
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
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_set_auditors(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    Ok(())
}

#[ic_cdk::update]
fn validate2_admin_set_auditors(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_update_bucket(args: UpdateBucketInput) -> Result<(), String> {
    store::state::validate_update(&args)
}

#[ic_cdk::update]
fn validate2_admin_update_bucket(args: UpdateBucketInput) -> Result<String, String> {
    store::state::validate_update(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_add_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_remove_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_add_auditors(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_remove_auditors(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

fn pretty_format<T>(data: &T) -> Result<String, String>
where
    T: CandidType,
{
    let val = IDLValue::try_from_candid_type(data).map_err(|err| format!("{err:?}"))?;
    let doc = pp_value(7, &val);

    Ok(format!("{}", doc.pretty(120)))
}
