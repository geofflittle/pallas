pub mod data;
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub(crate) mod dijkstra_evaluator;
pub mod error;
mod evaluator;
pub mod script_context;
pub mod to_plutus_data;
pub mod tx;

use error::Error;
use pallas_traverse::MultiEraTx;
use script_context::SlotConfig;

use crate::utils::{MultiEraProtocolParameters, UtxoMap};

pub type EvalReport = Vec<tx::TxEvalResult>;

/// The result of each redeemer of a transaction of any era from Alonzo on.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub type MultiEraEvalReport = Vec<tx::MultiEraTxEvalResult>;

pub fn evaluate_tx(
    tx: &MultiEraTx,
    pparams: &MultiEraProtocolParameters,
    utxos: &UtxoMap,
    slot_config: &SlotConfig,
) -> Result<EvalReport, Error> {
    tx::eval_tx(tx, pparams, utxos, slot_config)
}

/// Evaluates the scripts of a transaction of any era from Alonzo on.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub fn evaluate_multi_era_tx(
    tx: &MultiEraTx,
    pparams: &MultiEraProtocolParameters,
    utxos: &UtxoMap,
    slot_config: &SlotConfig,
) -> Result<MultiEraEvalReport, Error> {
    tx::eval_multi_era_tx(tx, pparams, utxos, slot_config)
}

/// Evaluates the scripts of a Dijkstra transaction, converting slots to times with `time`.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub fn evaluate_tx_with_time(
    tx: &MultiEraTx,
    pparams: &MultiEraProtocolParameters,
    utxos: &UtxoMap,
    time: &crate::utils::DijkstraTimeContext,
) -> Result<MultiEraEvalReport, Error> {
    let results = dijkstra::eval_tx_with_mode(tx, pparams, utxos, time, false)?;
    Ok(results.into_iter().map(|(result, _)| result).collect())
}

/// [`estimate_tx`], converting slots to times with `time`.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub fn estimate_tx_with_time(
    tx: &MultiEraTx,
    pparams: &MultiEraProtocolParameters,
    utxos: &UtxoMap,
    time: &crate::utils::DijkstraTimeContext,
) -> Result<MultiEraEvalReport, Error> {
    let results = estimate_tx_calls_with_time(tx, pparams, utxos, time)?;
    Ok(results.into_iter().map(|(result, _)| result).collect())
}

/// [`estimate_tx_calls`], converting slots to times with `time`.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub fn estimate_tx_calls_with_time(
    tx: &MultiEraTx,
    pparams: &MultiEraProtocolParameters,
    utxos: &UtxoMap,
    time: &crate::utils::DijkstraTimeContext,
) -> Result<Vec<(tx::MultiEraTxEvalResult, Vec<Builtin>)>, Error> {
    dijkstra::eval_tx_with_mode(tx, pparams, utxos, time, true)
}

/// The arguments each redeemer's script is applied to in [`estimate_tx_calls_with_time`], in its order.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub fn redeemer_arguments(
    tx: &MultiEraTx,
    pparams: &MultiEraProtocolParameters,
    utxos: &UtxoMap,
    time: &crate::utils::DijkstraTimeContext,
) -> Result<Vec<tx::RedeemerArguments>, Error> {
    let runs = dijkstra::runs(tx, pparams, utxos, time, true)?;
    let arguments = runs.into_iter().map(|run| tx::RedeemerArguments {
        sub_transaction: run.sub,
        tag: run.tag.into(),
        index: run.index,
        language: run.language,
        args: run.args,
    });
    Ok(arguments.collect())
}

/// The execution units each redeemer of a Dijkstra transaction uses, each run under the protocol's transaction limit.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub fn estimate_tx(
    tx: &MultiEraTx,
    pparams: &MultiEraProtocolParameters,
    utxos: &UtxoMap,
    slot_config: &SlotConfig,
) -> Result<MultiEraEvalReport, Error> {
    let estimates = estimate_tx_calls(tx, pparams, utxos, slot_config)?;
    Ok(estimates.into_iter().map(|(result, _)| result).collect())
}

/// [`estimate_tx`], with the builtins each redeemer's evaluation called in order of first call.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub fn estimate_tx_calls(
    tx: &MultiEraTx,
    pparams: &MultiEraProtocolParameters,
    utxos: &UtxoMap,
    slot_config: &SlotConfig,
) -> Result<Vec<(tx::MultiEraTxEvalResult, Vec<Builtin>)>, Error> {
    dijkstra::estimate_tx(tx, pparams, utxos, slot_config)
}

/// A builtin function of the Dijkstra evaluator.
#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
pub use dijkstra_evaluator::Builtin;

#[cfg(all(test, feature = "unstable", pallas_dijkstra_evaluator))]
mod dijkstra_evaluation_tests;

#[cfg(all(feature = "unstable", pallas_dijkstra_evaluator))]
mod dijkstra;

#[cfg(test)]
mod parameter_update_tests;

#[cfg(test)]
mod v3_deposit_context_tests;

#[cfg(all(test, feature = "unstable", pallas_dijkstra_evaluator))]
mod registration_evaluator_tests;

#[cfg(all(test, feature = "unstable", pallas_dijkstra_evaluator))]
mod dijkstra_v4_tests;

#[cfg(all(test, feature = "unstable", pallas_dijkstra_evaluator))]
mod dijkstra_v3_builtin_tests;
