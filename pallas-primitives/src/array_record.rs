//! Decoding of structs written as an array of their fields in order.

/// Implements `Decode` for a struct from its fields in array order, reading an array of either length form that holds exactly those fields, where a field marked `#[optional]` is a trailing member the array may leave out.
macro_rules! decode_array_record {
    (@field $a:ident, $ctx:ident, optional $field:ident) => {
        $a.decode_optional($ctx)?
    };
    (@field $a:ident, $ctx:ident, $field:ident) => {
        $a.decode_with($ctx)?
    };
    (@field $a:ident, $ctx:ident, $field:ident, $by:path) => {
        $a.decode_by($ctx, $by)?
    };
    (@impl $name:ident $(<$lt:lifetime>)?, |$a:ident, $ctx:ident| $value:expr) => {
        impl<'bytes $(: $lt, $lt)?, Ctx> pallas_codec::minicbor::Decode<'bytes, Ctx>
            for $name $(<$lt>)?
        {
            fn decode(
                d: &mut pallas_codec::minicbor::Decoder<'bytes>,
                $ctx: &mut Ctx,
            ) -> Result<$name $(<$lt>)?, pallas_codec::minicbor::decode::Error> {
                pallas_codec::utils::decode_array(d, |$a| Ok($value))
            }
        }
    };
    (
        $name:ident $(<$lt:lifetime>)? {
            $( $(#[$marker:ident])? $field:ident $(: $by:path)? ),+ $(,)?
        }
    ) => {
        $crate::array_record::decode_array_record!(@impl $name $(<$lt>)?, |a, ctx| $name {
            $(
                $field: $crate::array_record::decode_array_record!(
                    @field a, ctx, $($marker)? $field $(, $by)?
                ),
            )+
        });
    };
    (
        $name:ident $(<$lt:lifetime>)? (
            $( $(#[$marker:ident])? $field:ident $(: $by:path)? ),+ $(,)?
        )
    ) => {
        $crate::array_record::decode_array_record!(@impl $name $(<$lt>)?, |a, ctx| $name(
            $(
                $crate::array_record::decode_array_record!(
                    @field a, ctx, $($marker)? $field $(, $by)?
                ),
            )+
        ));
    };
}

pub(crate) use decode_array_record;

#[cfg(test)]
mod tests {
    use crate::alonzo::TransactionOutput;
    use crate::alonzo::{BootstrapWitness, VKeyWitness};
    use crate::babbage::OperationalCert;
    use crate::conway::{
        Anchor, Constitution, DRepVotingThresholds, ExUnitPrices, GovActionId,
        PoolVotingThresholds, VotingProcedure,
    };
    use crate::flat_enum::decode_all;
    use crate::{ExUnits, PoolMetadata, TransactionInput, VrfCert};
    use pallas_codec::minicbor;

    const HASH32: &str = "58201111111111111111111111111111111111111111111111111111111111111111";
    const RATIONAL: &str = "d81e820102";
    const ANCHOR: &str =
        "826361626358201111111111111111111111111111111111111111111111111111111111111111";

    /// The array of `elements` in the definite form when `definite`, the indefinite form otherwise.
    fn array(elements: &[&str], definite: bool) -> Vec<u8> {
        let (open, close) = if definite {
            (format!("{:02x}", 0x80 + elements.len()), "")
        } else {
            ("9f".to_string(), "ff")
        };
        hex::decode(format!("{open}{}{close}", elements.concat())).unwrap()
    }

    /// Asserts that `T` decodes the array of `elements` and refuses it with one element more or one fewer, in either length form.
    fn assert_holds_exactly<T>(elements: &[&str])
    where
        T: for<'b> minicbor::Decode<'b, ()> + std::fmt::Debug,
    {
        let name = std::any::type_name::<T>();
        let with_extra = [elements, &["00"]].concat();
        let short = &elements[..elements.len() - 1];
        for definite in [true, false] {
            decode_all::<T>(&array(elements, definite));
            for (case, elements, reason) in [
                ("an extra element", &with_extra[..], "more elements than"),
                ("its last element missing", short, "required field"),
            ] {
                let bytes = array(elements, definite);
                match minicbor::decode::<T>(&bytes) {
                    Ok(value) => panic!("{name} with {case} must be refused, read as {value:?}"),
                    Err(error) => assert!(
                        error.to_string().contains(reason),
                        "{name} with {case} was refused for some other reason: {error}"
                    ),
                }
            }
        }
    }

    #[test]
    fn a_shared_record_holds_exactly_its_fields() {
        assert_holds_exactly::<ExUnits>(&["01", "02"]);
        assert_holds_exactly::<TransactionInput>(&[HASH32, "00"]);
        assert_holds_exactly::<VrfCert>(&["4100", "4101"]);
        assert_holds_exactly::<PoolMetadata>(&["63616263", HASH32]);
        assert_holds_exactly::<VKeyWitness>(&["4100", "4101"]);
        assert_holds_exactly::<BootstrapWitness>(&["4100", "4101", "4102", "4103"]);
        assert_holds_exactly::<OperationalCert>(&["4100", "01", "02", "4103"]);
        assert_holds_exactly::<PoolVotingThresholds>(&[RATIONAL; 5]);
        assert_holds_exactly::<DRepVotingThresholds>(&[RATIONAL; 10]);
        assert_holds_exactly::<ExUnitPrices>(&[RATIONAL, RATIONAL]);
        assert_holds_exactly::<Anchor>(&["63616263", HASH32]);
        assert_holds_exactly::<GovActionId>(&[HASH32, "00"]);
        assert_holds_exactly::<VotingProcedure>(&["00", ANCHOR]);
        assert_holds_exactly::<VotingProcedure>(&["00", "f6"]);
        assert_holds_exactly::<Constitution>(&[ANCHOR, "f6"]);
    }

    #[test]
    fn a_legacy_output_may_leave_out_its_datum_hash_and_nothing_else() {
        for definite in [true, false] {
            let output: TransactionOutput = decode_all(&array(&["4100", "01", HASH32], definite));
            assert!(output.datum_hash.is_some());
            let output: TransactionOutput = decode_all(&array(&["4100", "01"], definite));
            assert_eq!(output.datum_hash, None);
            for elements in [
                &["4100"][..],
                &["4100", "01", "f6"][..],
                &["4100", "01", HASH32, "00"][..],
            ] {
                assert!(
                    minicbor::decode::<TransactionOutput>(&array(elements, definite)).is_err(),
                    "{elements:?} must be refused"
                );
            }
        }
    }
}
