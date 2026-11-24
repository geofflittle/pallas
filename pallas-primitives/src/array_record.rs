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

/// The array of `elements` in the definite form when `definite`, the indefinite form otherwise.
#[cfg(test)]
fn array_of(elements: &[&[u8]], definite: bool) -> &'static [u8] {
    let mut e = pallas_codec::minicbor::Encoder::new(Vec::new());
    if definite {
        e.array(elements.len() as u64).unwrap();
    } else {
        e.begin_array().unwrap();
    }
    for element in elements {
        e.writer_mut().extend_from_slice(element);
    }
    if !definite {
        e.end().unwrap();
    }
    Box::leak(e.into_writer().into_boxed_slice())
}

/// Asserts that `T` decodes the definite array `encoding` and refuses it with one element more or one fewer, in either length form.
#[cfg(test)]
pub(crate) fn assert_holds_exactly<T>(encoding: &[u8])
where
    T: pallas_codec::minicbor::Decode<'static, ()> + std::fmt::Debug,
{
    use pallas_codec::minicbor;

    let name = std::any::type_name::<T>();
    let mut d = minicbor::Decoder::new(encoding);
    let len = d.array().unwrap().expect("a definite array");
    let mut elements = Vec::new();
    for _ in 0..len {
        let start = d.position();
        d.skip().unwrap();
        elements.push(&encoding[start..d.position()]);
    }
    assert_eq!(d.position(), encoding.len(), "{name} is one array");
    let with_extra = [&elements[..], &[&[0x00][..]]].concat();
    let short = &elements[..elements.len() - 1];
    for definite in [true, false] {
        crate::flat_enum::decode_all::<T>(array_of(&elements, definite));
        for (case, elements, reason) in [
            ("an extra element", &with_extra[..], "more elements than"),
            ("its last element missing", short, "required field"),
        ] {
            match minicbor::decode::<T>(array_of(elements, definite)) {
                Ok(value) => panic!("{name} with {case} must be refused, read as {value:?}"),
                Err(error) => assert!(
                    error.to_string().contains(reason),
                    "{name} with {case} was refused for some other reason: {error}"
                ),
            }
        }
    }
}

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
        T: minicbor::Decode<'static, ()> + std::fmt::Debug,
    {
        super::assert_holds_exactly::<T>(Box::leak(array(elements, true).into_boxed_slice()));
    }

    #[test]
    fn a_shared_record_holds_exactly_its_fields() {
        let bytes = |size: usize| format!("58{size:02x}{}", "11".repeat(size));
        let (vkey, signature) = (bytes(32), bytes(64));
        assert_holds_exactly::<ExUnits>(&["01", "02"]);
        assert_holds_exactly::<TransactionInput>(&[HASH32, "00"]);
        assert_holds_exactly::<VrfCert>(&[&signature, &bytes(80)]);
        assert_holds_exactly::<PoolMetadata>(&["63616263", HASH32]);
        assert_holds_exactly::<VKeyWitness>(&[&vkey, &signature]);
        assert_holds_exactly::<BootstrapWitness>(&[&vkey, &signature, &vkey, "4103"]);
        assert_holds_exactly::<OperationalCert>(&[&vkey, "01", "02", &signature]);
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
