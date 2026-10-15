//! Matches the phase two `Error` exhaustively over the variants the base branch has.

use pallas_validate::phase2::error::Error;

fn name(error: &Error) -> &'static str {
    match error {
        Error::Address { .. } => "Address",
        Error::BadWithdrawalAddress { .. } => "BadWithdrawalAddress",
        Error::FlatDecode { .. } => "FlatDecode",
        Error::FragmentDecode { .. } => "FragmentDecode",
        Error::Machine { .. } => "Machine",
        Error::NativeScriptPhaseTwo { .. } => "NativeScriptPhaseTwo",
        Error::NoRedeemers { .. } => "NoRedeemers",
        Error::RequiredRedeemersMismatch { .. } => "RequiredRedeemersMismatch",
        Error::ExtraneousRedeemer { .. } => "ExtraneousRedeemer",
        Error::ResolvedInputNotFound { .. } => "ResolvedInputNotFound",
        Error::NonScriptWithdrawal { .. } => "NonScriptWithdrawal",
        Error::NonScriptStakeCredential { .. } => "NonScriptStakeCredential",
        Error::NoGuardrailScriptForProcedure { .. } => "NoGuardrailScriptForProcedure",
        Error::CostModelNotFound { .. } => "CostModelNotFound",
        Error::WrongEra { .. } => "WrongEra",
        Error::DecodeError { .. } => "DecodeError",
        Error::ByronAddressNotAllowed { .. } => "ByronAddressNotAllowed",
        Error::InlineDatumNotAllowed { .. } => "InlineDatumNotAllowed",
        Error::ScriptAndInputRefNotAllowed { .. } => "ScriptAndInputRefNotAllowed",
        Error::NoPaymentCredential { .. } => "NoPaymentCredential",
        Error::MissingRequiredDatum { .. } => "MissingRequiredDatum",
        Error::MissingRequiredScript { .. } => "MissingRequiredScript",
        Error::MissingRequiredInlineDatumOrHash { .. } => "MissingRequiredInlineDatumOrHash",
        Error::UnsupportedCertificateType { .. } => "UnsupportedCertificateType",
        Error::RedeemerError { .. } => "RedeemerError",
        Error::MissingScriptForRedeemer { .. } => "MissingScriptForRedeemer",
        Error::ApplyParamsError { .. } => "ApplyParamsError",
        Error::SlotTooFarInThePast { .. } => "SlotTooFarInThePast",
        Error::ScriptContextBuildError { .. } => "ScriptContextBuildError",
    }
}

fn main() {
    println!("{}", name(&Error::WrongEra()));
}
