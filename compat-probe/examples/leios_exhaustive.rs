// A consumer outside pallas matching the two public leios enums with no
// wildcard arm.
//
// Expected verdict before the change: compiles, which is the report that
// these enums let an outside caller name every variant and so freeze their
// variant lists at release.
// Expected verdict after the change: two E0004 errors, one per enum, saying
// a wildcard is necessary because the enum is marked non_exhaustive.

use pallas_traverse::leios::{Error, PendingAnnouncement};

fn describe_error(error: &Error) -> u8 {
    match error {
        Error::InvalidBody(_) => 0,
        Error::BodySize { .. } => 1,
        Error::BodyHash { .. } => 2,
        Error::TxCount { .. } => 3,
        Error::Envelope { .. } => 4,
        Error::TxSize { .. } => 5,
        Error::TxHash { .. } => 6,
        Error::TxDecode { .. } => 7,
        Error::CertifiesNothing { .. } => 8,
        Error::CertifiesUnknown { .. } => 9,
        Error::CertifiesAndCarries { .. } => 10,
        Error::NotCertifying { .. } => 11,
        Error::NotLeiosEra { .. } => 12,
        Error::InvalidBlock(_) => 13,
    }
}

fn describe_pending(pending: &PendingAnnouncement) -> u8 {
    match pending {
        PendingAnnouncement::Nothing => 0,
        PendingAnnouncement::Waiting(_) => 1,
        PendingAnnouncement::Unknown => 2,
    }
}

fn main() {
    assert_eq!(describe_error(&Error::InvalidBody(String::new())), 0);
    assert_eq!(describe_pending(&PendingAnnouncement::Nothing), 0);
    println!("an outside caller can name every leios variant");
}
