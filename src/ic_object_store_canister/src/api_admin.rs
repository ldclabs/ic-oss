use candid::{pretty::candid::value::pp_value, CandidType, IDLValue, Principal};
use ic_oss_types::validate_principals;
use std::collections::BTreeSet;

use crate::{is_controller, store};

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|s| {
        s.managers.extend(args);
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_remove_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|s| {
        s.managers.retain(|v| !args.contains(v));
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_auditors(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|s| {
        s.auditors.extend(args);
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_remove_auditors(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|s| {
        s.auditors.retain(|v| !args.contains(v));
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_clear() -> Result<(), String> {
    store::state::clear();
    Ok(())
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

#[ic_cdk::update]
fn validate_admin_clear() -> Result<String, String> {
    Ok("ok".to_string())
}

fn pretty_format<T>(data: &T) -> Result<String, String>
where
    T: CandidType,
{
    let val = IDLValue::try_from_candid_type(data).map_err(|err| format!("{err:?}"))?;
    let doc = pp_value(7, &val);

    Ok(format!("{}", doc.pretty(120)))
}
