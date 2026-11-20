//! The verdict pair for one transaction and the per kind table built from them.

use std::collections::BTreeMap;

/// The ledger decoder's verdict on one transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ledger {
    Accepts,
    Refuses(String),
}

/// The pallas-validate phase one verdict on one transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validator {
    Accepts,
    RefusesMalformed(String),
    RefusesOther(String),
}

/// The two verdicts on one transaction, as the table counts them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    LedgerAccepts(Validator),
    BothRefuse,
    ValidatorRefusesOther(String),
    ValidatorAccepts,
}

pub fn classify(ledger: &Ledger, validator: &Validator) -> Outcome {
    match (ledger, validator) {
        (Ledger::Accepts, v) => Outcome::LedgerAccepts(v.clone()),
        (Ledger::Refuses(_), Validator::RefusesMalformed(_)) => Outcome::BothRefuse,
        (Ledger::Refuses(_), Validator::RefusesOther(e)) => {
            Outcome::ValidatorRefusesOther(e.clone())
        }
        (Ledger::Refuses(_), Validator::Accepts) => Outcome::ValidatorAccepts,
    }
}

/// The counts of one kind.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Counts {
    pub refused_by_ledger: usize,
    pub refused_malformed: usize,
    pub refused_other: usize,
    pub accepted: usize,
    pub ledger_accepts: usize,
}

/// The per kind table.
#[derive(Debug, Default)]
pub struct Table {
    pub kinds: BTreeMap<String, Counts>,
}

impl Table {
    pub fn add(&mut self, kind: &str, outcome: &Outcome) {
        let c = self.kinds.entry(kind.to_owned()).or_default();
        match outcome {
            Outcome::LedgerAccepts(_) => c.ledger_accepts += 1,
            Outcome::BothRefuse => {
                c.refused_by_ledger += 1;
                c.refused_malformed += 1;
            }
            Outcome::ValidatorRefusesOther(_) => {
                c.refused_by_ledger += 1;
                c.refused_other += 1;
            }
            Outcome::ValidatorAccepts => {
                c.refused_by_ledger += 1;
                c.accepted += 1;
            }
        }
    }

    pub fn failed(&self) -> bool {
        self.kinds.values().any(|c| c.accepted > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn malformed() -> Validator {
        Validator::RefusesMalformed("DijkstraMalformed(Rational)".into())
    }

    fn other() -> Validator {
        Validator::RefusesOther("PostAlonzo(FeeBelowMin)".into())
    }

    fn refuses() -> Ledger {
        Ledger::Refuses("Size mismatch when decoding Rational".into())
    }

    #[test]
    fn both_accepting_is_no_failure() {
        let outcome = classify(&Ledger::Accepts, &Validator::Accepts);
        assert_eq!(outcome, Outcome::LedgerAccepts(Validator::Accepts));
        let mut table = Table::default();
        table.add("base", &outcome);
        assert!(!table.failed());
        assert_eq!(table.kinds["base"].ledger_accepts, 1);
        assert_eq!(table.kinds["base"].refused_by_ledger, 0);
    }

    #[test]
    fn validator_refusing_what_the_ledger_accepts_is_no_failure_here() {
        let mut table = Table::default();
        table.add("k", &classify(&Ledger::Accepts, &other()));
        assert!(!table.failed());
    }

    #[test]
    fn a_malformed_refusal_agrees() {
        let outcome = classify(&refuses(), &malformed());
        assert_eq!(outcome, Outcome::BothRefuse);
        let mut table = Table::default();
        table.add("k", &outcome);
        assert!(!table.failed());
        let c = &table.kinds["k"];
        assert_eq!(
            (
                c.refused_by_ledger,
                c.refused_malformed,
                c.refused_other,
                c.accepted
            ),
            (1, 1, 0, 0)
        );
    }

    #[test]
    fn another_reason_is_its_own_column_and_not_agreement() {
        let outcome = classify(&refuses(), &other());
        assert_eq!(
            outcome,
            Outcome::ValidatorRefusesOther("PostAlonzo(FeeBelowMin)".into())
        );
        let mut table = Table::default();
        table.add("k", &outcome);
        let c = &table.kinds["k"];
        assert_eq!(
            (
                c.refused_by_ledger,
                c.refused_malformed,
                c.refused_other,
                c.accepted
            ),
            (1, 0, 1, 0)
        );
        assert!(!table.failed());
    }

    #[test]
    fn ledger_refusing_what_the_validator_accepts_fails() {
        let outcome = classify(&refuses(), &Validator::Accepts);
        assert_eq!(outcome, Outcome::ValidatorAccepts);
        let mut table = Table::default();
        table.add("k", &classify(&refuses(), &malformed()));
        table.add("k", &outcome);
        assert!(table.failed());
        let c = &table.kinds["k"];
        assert_eq!(
            (
                c.refused_by_ledger,
                c.refused_malformed,
                c.refused_other,
                c.accepted
            ),
            (2, 1, 0, 1)
        );
    }
}
