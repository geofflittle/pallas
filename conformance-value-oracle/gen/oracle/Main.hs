{-# LANGUAGE DataKinds #-}
{-# LANGUAGE FlexibleContexts #-}
{-# LANGUAGE GADTs #-}
{-# LANGUAGE LambdaCase #-}
{-# LANGUAGE NamedFieldPuns #-}
{-# LANGUAGE OverloadedStrings #-}
{-# LANGUAGE RecordWildCards #-}
{-# LANGUAGE ScopedTypeVariables #-}
{-# LANGUAGE TypeApplications #-}

-- | Generates Dijkstra ledger values and decodes values with the ledger.
module Main (main) where

import Cardano.Crypto.Hash (hashToTextAsHex)
import Cardano.Ledger.Address (DirectDeposits (..), Withdrawals (..))
import Cardano.Ledger.Allegra.Scripts (ValidityInterval (..))
import Cardano.Ledger.Alonzo.Scripts (AlonzoEraScript (..), AlonzoScript (..), plutusScriptBinary, plutusScriptLanguage)
import Cardano.Ledger.Alonzo.Tx (AlonzoEraTx (..), IsPhase2Valid (..))
import Cardano.Ledger.Alonzo.TxAuxData (AlonzoTxAuxDataRaw (..))
import Cardano.Ledger.Alonzo.TxWits (AlonzoEraTxWits (..), unRedeemers, unTxDats)
import Cardano.Ledger.Babbage.TxOut (BabbageEraTxOut (..))
import Cardano.Ledger.Conway.Governance (GovAction (..), ProposalProcedure (..))
import Cardano.Ledger.Dijkstra.Scripts (DijkstraNativeScriptRaw (..))
import Cardano.Ledger.Dijkstra.TxCert (DijkstraTxCert (..))
import Cardano.Ledger.Mary.Value (MaryValue (..))
import Cardano.Ledger.Plutus.Data (Data, Datum (..), binaryDataToData, getPlutusData)
import Cardano.Ledger.Plutus.Language (PlutusBinary (..))
import Cardano.Ledger.State (StakePoolParams (..))
import Cardano.Ledger.TxIn (TxIn)
import qualified Data.ByteString.Short as SBS
import qualified Data.List.NonEmpty as NE
import qualified PlutusLedgerApi.V1 as PV1
import Cardano.Ledger.BaseTypes (Network (..), StrictMaybe (..))
import Cardano.Ledger.Binary (
  Annotator,
  DecCBOR (..),
  EncCBOR,
  Sized (..),
  Version,
  decodeFull',
  decodeFullAnnotator,
  encodeFixedSized,
  serialize',
 )
import Cardano.Ledger.Block (Block (..), BlockHeaderVersionInfo (..))
import Cardano.Ledger.Coin (Coin (..))
import Cardano.Ledger.Conway.Governance (VotingProcedures (..))
import Cardano.Ledger.Core
import Cardano.Ledger.Dijkstra (DijkstraEra)
import Cardano.Ledger.Dijkstra.BlockBody (DijkstraEraBlockBody (..))
import Cardano.Ledger.Dijkstra.HuddleSpec (dijkstraCDDL)
import Cardano.Ledger.Dijkstra.Scripts (AccountBalanceIntervals (..))
import Cardano.Ledger.Dijkstra.TxBody (DijkstraTxBodyRaw (..))
import Cardano.Ledger.Mary.Value (MultiAsset (..))
import Cardano.Ledger.MemoBytes (getMemoRawType)
import Cardano.Ledger.Plutus.Language (Language (..))
import Cardano.Protocol.Crypto (StandardCrypto)
import qualified Cardano.Protocol.Leios.BlockHeader as Leios
import Cardano.Slotting.Block (BlockNo (..))
import Cardano.Slotting.Slot (SlotNo (..))
import Codec.CBOR.Cuddle.CBOR.Gen (generateFromName)
import Codec.CBOR.Cuddle.CBOR.Validator (validateCBOR)
import Codec.CBOR.Cuddle.CBOR.Validator.Trace (Evidenced (..), SValidity (..))
import Codec.CBOR.Cuddle.CDDL (Name (..))
import Codec.CBOR.Cuddle.CDDL.Custom.Generator (runCBORGen)
import Codec.CBOR.Cuddle.IndexMappable (mapIndex)
import qualified Codec.CBOR.Read as CBOR
import qualified Codec.CBOR.Term as CBOR
import qualified Codec.CBOR.Write as CBOR
import Data.Aeson ((.=))
import qualified Data.Aeson as Aeson
import qualified Data.Aeson.Key as Key
import qualified Data.ByteString as BS
import qualified Data.ByteString.Base16 as B16
import qualified Data.ByteString.Char8 as BS8
import qualified Data.ByteString.Lazy as BSL
import qualified Data.ByteString.Lazy.Char8 as BSL8
import Data.Foldable (toList)
import qualified Data.Map.Strict as Map
import qualified Data.OMap.Strict as OMap
import qualified Data.OSet.Strict as OSet
import qualified Data.Set as Set
import qualified Data.Text as T
import qualified Data.Text.Encoding as T
import Lens.Micro ((^.))
import System.Environment (getArgs)
import System.IO (hFlush, stdout)
import Test.AntiGen (runAntiGen)
import Test.Cardano.Ledger.Binary.Cuddle (HuddleEnv (..), resolveHuddle, toGenConfig)
import Test.Cardano.Ledger.Dijkstra.Arbitrary (
  genSmallDijkstraCertBlockBody,
  genSmallDijkstraTxsBlockBody,
 )
import Test.Cardano.Protocol.Leios.BlockHeader.Arbitrary ()
import Test.QuickCheck (Gen, arbitrary)
import Test.QuickCheck.Gen (unGen)
import Test.QuickCheck.Random (mkQCGen)

type LeiosBlock = Block (Leios.Header StandardCrypto) DijkstraEra

data Kind = KTx | KBody | KWits | KPpu | KBlock

parseKind :: String -> Kind
parseKind = \case
  "tx" -> KTx
  "body" -> KBody
  "wits" -> KWits
  "ppu" -> KPpu
  "block" -> KBlock
  k -> error ("unknown kind " <> k)

ruleOf :: Kind -> T.Text
ruleOf = \case
  KTx -> "mempool_transaction"
  KBody -> "transaction_body"
  KWits -> "transaction_witness_set"
  KPpu -> "protocol_param_update"
  KBlock -> "block"

ver :: Version
ver = eraProtVerHigh @DijkstraEra

hexT :: BS.ByteString -> T.Text
hexT = T.decodeUtf8 . B16.encode

-- Generation ------------------------------------------------------------------

arbBytes :: Kind -> Int -> Gen BS.ByteString
arbBytes k i = case k of
  KTx -> serialize' ver <$> arbitrary @(Tx TopTx DijkstraEra)
  KBody -> serialize' ver <$> arbitrary @(TxBody TopTx DijkstraEra)
  KWits -> serialize' ver <$> arbitrary @(TxWits DijkstraEra)
  KPpu -> serialize' ver <$> arbitrary @(PParamsUpdate DijkstraEra)
  KBlock -> do
    h <- arbitrary @(Leios.Header StandardCrypto)
    b <- if even i then genSmallDijkstraTxsBlockBody else genSmallDijkstraCertBlockBody
    pure $ serialize' ver (Block h b :: LeiosBlock)

cddlBytes :: HuddleEnv -> Kind -> Gen BS.ByteString
cddlBytes env k = do
  term <- runAntiGen . runCBORGen (toGenConfig env) . generateFromName $ Name (ruleOf k)
  pure $ CBOR.toStrictByteString (CBOR.encodeTerm term)

validity :: HuddleEnv -> Kind -> BS.ByteString -> Aeson.Value
validity env k bs = case validateCBOR bs (Name (ruleOf k)) (mapIndex (heRoot env)) of
  Left e -> Aeson.String (T.pack ("error: " <> take 300 (show e)))
  Right (Evidenced SValid _) -> Aeson.Bool True
  Right (Evidenced SInvalid _) -> Aeson.Bool False

runGen :: Kind -> String -> Int -> Int -> IO ()
runGen k source n seed = do
  root <- either error pure (resolveHuddle dijkstraCDDL)
  let env = HuddleEnv {heTwiddle = source == "cddl", heRoot = root}
      gen i = case source of
        "arb" -> arbBytes k i
        "cddl" -> cddlBytes env k
        "cddl-plain" -> cddlBytes env k
        s -> error ("unknown source " <> s)
  mapM_
    ( \i -> do
        let bs = unGen (gen i) (mkQCGen (seed + i)) (5 + i `mod` 46)
        BSL8.putStrLn . Aeson.encode $
          Aeson.object
            [ "i" .= i
            , "source" .= source
            , "hex" .= hexT bs
            , "cddl_valid" .= validity env k bs
            ]
        hFlush stdout
    )
    [0 .. n - 1]

-- Oracle ----------------------------------------------------------------------

sm :: (a -> Aeson.Value) -> StrictMaybe a -> Aeson.Value
sm _ SNothing = Aeson.Null
sm f (SJust x) = f x

present :: StrictMaybe a -> Bool
present SNothing = False
present (SJust _) = True

bodyFacts :: TxBody TopTx DijkstraEra -> [(T.Text, Aeson.Value)]
bodyFacts body = case getMemoRawType body of
  DijkstraTxBodyRaw
    { dtbrSpendInputs
    , dtbrCollateralInputs
    , dtbrReferenceInputs
    , dtbrOutputs
    , dtbrCollateralReturn
    , dtbrTotalCollateral
    , dtbrCerts
    , dtbrWithdrawals
    , dtbrFee
    , dtbrVldt
    , dtbrGuards
    , dtbrMint
    , dtbrScriptIntegrityHash
    , dtbrAuxDataHash
    , dtbrNetworkId
    , dtbrVotingProcedures
    , dtbrProposalProcedures
    , dtbrCurrentTreasuryValue
    , dtbrTreasuryDonation
    , dtbrSubTransactions
    , dtbrRequiredTopLevelGuards
    , dtbrDirectDeposits
    , dtbrAccountBalanceIntervals
    , dtbrStartingAccountBalanceIntervals
    } ->
      let MultiAsset mint = dtbrMint
          coin (Coin n) = Aeson.toJSON n
          slot (SlotNo s) = Aeson.toJSON s
       in [ ("fee", coin dtbrFee)
          , ("ttl", sm slot (invalidHereafter dtbrVldt))
          , ("validity_start", sm slot (invalidBefore dtbrVldt))
          , ("inputs", Aeson.toJSON (Set.size dtbrSpendInputs))
          , ("outputs", Aeson.toJSON (length dtbrOutputs))
          , ("outputs_coin", Aeson.toJSON (sum [unCoin (sizedValue o ^. coinTxOutL) | o <- toList dtbrOutputs]))
          , ("certs", Aeson.toJSON (OSet.size dtbrCerts))
          , ("withdrawals", Aeson.toJSON (Map.size (unWithdrawals dtbrWithdrawals)))
          , ("mint_policies", Aeson.toJSON (Map.size mint))
          , ("aux_data_hash", Aeson.toJSON (present dtbrAuxDataHash))
          , ("script_data_hash", Aeson.toJSON (present dtbrScriptIntegrityHash))
          , ("collateral", Aeson.toJSON (Set.size dtbrCollateralInputs))
          , ("guards", Aeson.toJSON (OSet.size dtbrGuards))
          , ("network_id", sm (\case Testnet -> Aeson.toJSON (0 :: Int); Mainnet -> Aeson.toJSON (1 :: Int)) dtbrNetworkId)
          , ("collateral_return", Aeson.toJSON (present dtbrCollateralReturn))
          , ("total_collateral", sm coin dtbrTotalCollateral)
          , ("reference_inputs", Aeson.toJSON (Set.size dtbrReferenceInputs))
          , ("voters", Aeson.toJSON (Map.size (unVotingProcedures dtbrVotingProcedures)))
          , ("proposals", Aeson.toJSON (OSet.size dtbrProposalProcedures))
          , ("treasury", sm coin dtbrCurrentTreasuryValue)
          , ("donation", coin dtbrTreasuryDonation)
          , ("sub_transactions", Aeson.toJSON (OMap.size dtbrSubTransactions))
          , ("required_top_level_guards", Aeson.toJSON (Map.size dtbrRequiredTopLevelGuards))
          , ("direct_deposits", Aeson.toJSON (Map.size (unDirectDeposits dtbrDirectDeposits)))
          , ("account_balance_intervals", Aeson.toJSON (Map.size (unAccountBalanceIntervals dtbrAccountBalanceIntervals)))
          , ("starting_account_balance_intervals", Aeson.toJSON (Map.size (unAccountBalanceIntervals dtbrStartingAccountBalanceIntervals)))
          ]

witsFacts :: TxWits DijkstraEra -> [(T.Text, Aeson.Value)]
witsFacts w =
  let scripts = Map.elems (w ^. scriptTxWitsL)
      lang l = length [() | PlutusScript p <- scripts, plutusScriptLanguage p == l]
   in [ ("vkey", Aeson.toJSON (Set.size (w ^. addrTxWitsL)))
      , ("bootstrap", Aeson.toJSON (Set.size (w ^. bootAddrTxWitsL)))
      , ("native", Aeson.toJSON (length [() | NativeScript _ <- scripts]))
      , ("plutus_v1", Aeson.toJSON (lang PlutusV1))
      , ("plutus_v2", Aeson.toJSON (lang PlutusV2))
      , ("plutus_v3", Aeson.toJSON (lang PlutusV3))
      , ("plutus_v4", Aeson.toJSON (lang PlutusV4))
      , ("datums", Aeson.toJSON (Map.size (unTxDats (w ^. datsTxWitsL))))
      , ("redeemers", Aeson.toJSON (Map.size (unRedeemers (w ^. rdmrsTxWitsL))))
      ]

prefixed :: T.Text -> [(T.Text, Aeson.Value)] -> [(T.Text, Aeson.Value)]
prefixed p = map (\(k, v) -> (p <> k, v))

txFacts :: Tx TopTx DijkstraEra -> [(T.Text, Aeson.Value)]
txFacts tx =
  prefixed "body." (bodyFacts (tx ^. bodyTxL))
    <> prefixed "wits." (witsFacts (tx ^. witsTxL))
    <> [("aux_data", Aeson.toJSON (present (tx ^. auxDataTxL)))]

-- | Each key of the ledger's own re-encoding of the update, with the value's CBOR.
ppuFacts :: PParamsUpdate DijkstraEra -> [(T.Text, Aeson.Value)]
ppuFacts ppu =
  case CBOR.deserialiseFromBytes CBOR.decodeTerm (BSL.fromStrict (serialize' ver ppu)) of
    Right (_, CBOR.TMap kvs) -> map entry kvs
    Right (_, CBOR.TMapI kvs) -> map entry kvs
    other -> [("unexpected", Aeson.String (T.pack (take 200 (show other))))]
  where
    entry (k, v) =
      ( "k" <> T.pack (showKey k)
      , Aeson.String (hexT (CBOR.toStrictByteString (CBOR.encodeTerm v)))
      )
    showKey (CBOR.TInt n) = show n
    showKey (CBOR.TInteger n) = show n
    showKey t = show t

blockFacts :: LeiosBlock -> [(T.Text, Aeson.Value)]
blockFacts (Block h b) =
  let hb = Leios.headerBody h
      BlockNo bn = Leios.hbBlockNo hb
      SlotNo sl = Leios.hbSlotNo hb
   in [ ("hash", Aeson.String (hashToTextAsHex (Leios.headerHash h)))
      , ("block_no", Aeson.toJSON bn)
      , ("slot", Aeson.toJSON sl)
      , ("prev_genesis", Aeson.toJSON (show (Leios.hbPrev hb) == "GenesisHash"))
      , ("body_size", Aeson.toJSON (Leios.hbBodySize hb))
      , ("body_hash", Aeson.String (hashToTextAsHex (Leios.hbBodyHash hb)))
      , ("contains_leios_cert", Aeson.toJSON (Leios.hbBlockBodyContainsLeiosCert hb))
      , ("eb_announcement", Aeson.toJSON (present (Leios.hbEbReferencesAnnouncement hb)))
      , ("protocol_major", Aeson.toJSON (bhviHighestSupportedMajorVersion (Leios.hbVersionInfo hb)))
      , ("txs", Aeson.toJSON (length (b ^. txSeqBlockBodyL)))
      , ("leios_certificate", Aeson.toJSON (present (b ^. leiosCertBlockBodyL)))
      , ("peras_certificate", Aeson.toJSON (present (b ^. perasCertBlockBodyL)))
      ]

-- Value dump ------------------------------------------------------------------

-- | A decoded value in the form the comparer reads, with each leaf the ledger's own encoding of it.
data D
  = DS T.Text
  | DC BS.ByteString
  | DNull
  | DB Bool
  | DL [D]
  | DSet [D]
  | DOSet [D]
  | DMap [(D, D)]
  | DOMap [(D, D)]
  | DO [(T.Text, D)]

toJ :: D -> Aeson.Value
toJ = \case
  DS t -> Aeson.String t
  DC bs -> Aeson.object ["cbor" .= hexT bs]
  DNull -> Aeson.Null
  DB b -> Aeson.Bool b
  DL xs -> Aeson.toJSON (map toJ xs)
  DSet xs -> Aeson.object ["set" .= map toJ xs]
  DOSet xs -> Aeson.object ["oset" .= map toJ xs]
  DMap kvs -> Aeson.object ["map" .= [[toJ k, toJ v] | (k, v) <- kvs]]
  DOMap kvs -> Aeson.object ["omap" .= [[toJ k, toJ v] | (k, v) <- kvs]]
  DO fs -> Aeson.object [Key.fromText k .= toJ v | (k, v) <- fs]

c :: EncCBOR a => a -> D
c = DC . serialize' ver

int :: Integral a => a -> D
int = DS . T.pack . show . toInteger

tagged :: T.Text -> [(T.Text, D)] -> D
tagged t fs = DO (("tag", DS t) : fs)

opt :: (a -> D) -> StrictMaybe a -> D
opt _ SNothing = DNull
opt f (SJust x) = f x

mapD :: (k -> D) -> (v -> D) -> Map.Map k v -> D
mapD f g m = DMap [(f k, g v) | (k, v) <- Map.toList m]

setD :: (a -> D) -> Set.Set a -> D
setD f = DSet . map f . Set.toList

osetD :: (a -> D) -> OSet.OSet a -> D
osetD f = DOSet . map f . toList

multiD :: MultiAsset -> D
multiD (MultiAsset m) = mapD c (mapD c c) m

plutusD :: PV1.Data -> D
plutusD = \case
  PV1.Constr i fs -> tagged "constr" [("ix", int i), ("fields", DL (map plutusD fs))]
  PV1.Map kvs -> tagged "map" [("entries", DL [DL [plutusD k, plutusD v] | (k, v) <- kvs])]
  PV1.List xs -> tagged "list" [("items", DL (map plutusD xs))]
  PV1.I n -> tagged "int" [("v", int n)]
  PV1.B b -> tagged "bytes" [("v", DS (hexT b))]

dataD :: Data DijkstraEra -> D
dataD = plutusD . getPlutusData

langName :: Language -> T.Text
langName = \case
  PlutusV1 -> "plutus_v1"
  PlutusV2 -> "plutus_v2"
  PlutusV3 -> "plutus_v3"
  PlutusV4 -> "plutus_v4"

binD :: PlutusBinary -> D
binD = DS . hexT . SBS.fromShort . unPlutusBinary

nativeD :: NativeScript DijkstraEra -> D
nativeD ns = case getMemoRawType ns of
  DijkstraRequireSignature h -> tagged "sig" [("hash", c h)]
  DijkstraRequireAllOf xs -> tagged "all" [("scripts", DL (map nativeD (toList xs)))]
  DijkstraRequireAnyOf xs -> tagged "any" [("scripts", DL (map nativeD (toList xs)))]
  DijkstraRequireMOf m xs -> tagged "m_of" [("m", int m), ("scripts", DL (map nativeD (toList xs)))]
  DijkstraTimeStart s -> tagged "start" [("slot", c s)]
  DijkstraTimeExpire s -> tagged "expire" [("slot", c s)]
  DijkstraRequireGuard cr -> tagged "guard" [("cred", c cr)]

scriptD :: Script DijkstraEra -> D
scriptD = \case
  NativeScript ns -> tagged "native" [("script", nativeD ns)]
  PlutusScript ps -> tagged (langName (plutusScriptLanguage ps)) [("bytes", binD (plutusScriptBinary ps))]

outD :: TxOut DijkstraEra -> D
outD o =
  let MaryValue coin ma = o ^. valueTxOutL
   in DO
        [ ("address", c (o ^. addrTxOutL))
        , ("coin", c coin)
        , ("assets", multiD ma)
        , ( "datum"
          , case o ^. datumTxOutL of
              NoDatum -> DNull
              DatumHash h -> tagged "hash" [("hash", c h)]
              Datum bd -> tagged "inline" [("data", dataD (binaryDataToData bd))]
          )
        , ("script", opt scriptD (o ^. referenceScriptTxOutL))
        ]

certD :: TxCert DijkstraEra -> D
certD = \case
  DijkstraTxCertPool (RegPool p) ->
    tagged
      "pool_reg"
      [ ("operator", c (sppId p))
      , ("vrf", c (sppVrf p))
      , ("bls", opt c (sppBlsKey p))
      , ("pledge", c (sppPledge p))
      , ("cost", c (sppCost p))
      , ("margin", c (sppMargin p))
      , ("account", c (sppAccountAddress p))
      , ("owners", setD c (sppOwners p))
      , ("relays", DL (map c (toList (sppRelays p))))
      , ("metadata", opt c (sppMetadata p))
      ]
  cert -> c cert

actionD :: GovAction DijkstraEra -> D
actionD = \case
  TreasuryWithdrawals ws pol ->
    tagged "treasury_withdrawals" [("withdrawals", mapD c c ws), ("policy", opt c pol)]
  UpdateCommittee prev rm add thr ->
    tagged
      "update_committee"
      [ ("prev", opt c prev)
      , ("remove", setD c rm)
      , ("add", mapD c c add)
      , ("threshold", c thr)
      ]
  a -> c a

proposalD :: ProposalProcedure DijkstraEra -> D
proposalD p =
  DO
    [ ("deposit", c (pProcDeposit p))
    , ("account", c (pProcReturnAddr p))
    , ("action", actionD (pProcGovAction p))
    , ("anchor", c (pProcAnchor p))
    ]

votesD :: VotingProcedures DijkstraEra -> D
votesD (VotingProcedures m) = mapD c (mapD c c) m

intervalsD :: AccountBalanceIntervals DijkstraEra -> D
intervalsD (AccountBalanceIntervals m) = mapD c c m

-- | The fields a top level body and a sub transaction body both hold, by name.
sharedD ::
  Set.Set TxIn ->
  Set.Set TxIn ->
  [Sized (TxOut DijkstraEra)] ->
  OSet.OSet (TxCert DijkstraEra) ->
  Withdrawals ->
  ValidityInterval ->
  [(T.Text, D)]
sharedD ins refs outs certs (Withdrawals ws) vldt =
  [ ("inputs", setD c ins)
  , ("reference_inputs", setD c refs)
  , ("outputs", DL (map (outD . sizedValue) outs))
  , ("certs", osetD certD certs)
  , ("withdrawals", mapD c c ws)
  , ("ttl", opt c (invalidHereafter vldt))
  , ("start", opt c (invalidBefore vldt))
  ]

bodyD :: TxBody l DijkstraEra -> D
bodyD body = case getMemoRawType body of
  DijkstraTxBodyRaw {..} ->
    DO $
      sharedD dtbrSpendInputs dtbrReferenceInputs (toList dtbrOutputs) dtbrCerts dtbrWithdrawals dtbrVldt
        <> [ ("fee", c dtbrFee)
           , ("collateral", setD c dtbrCollateralInputs)
           , ("collateral_return", opt (outD . sizedValue) dtbrCollateralReturn)
           , ("total_collateral", opt c dtbrTotalCollateral)
           , ("guards", osetD c dtbrGuards)
           , ("mint", multiD dtbrMint)
           , ("script_data_hash", opt c dtbrScriptIntegrityHash)
           , ("aux_hash", opt c dtbrAuxDataHash)
           , ("network", opt c dtbrNetworkId)
           , ("votes", votesD dtbrVotingProcedures)
           , ("proposals", osetD proposalD dtbrProposalProcedures)
           , ("treasury", opt c dtbrCurrentTreasuryValue)
           , ("donation", c dtbrTreasuryDonation)
           , ("sub_txs", DOMap [(c k, subTxD v) | (k, v) <- OMap.assocList dtbrSubTransactions])
           , ("top_guards", mapD c (opt dataD) dtbrRequiredTopLevelGuards)
           , ("deposits", mapD c c (unDirectDeposits dtbrDirectDeposits))
           , ("balance_intervals", intervalsD dtbrAccountBalanceIntervals)
           , ("starting_balance_intervals", intervalsD dtbrStartingAccountBalanceIntervals)
           ]
  DijkstraSubTxBodyRaw {..} ->
    DO $
      sharedD dstbrSpendInputs dstbrReferenceInputs (toList dstbrOutputs) dstbrCerts dstbrWithdrawals dstbrVldt
        <> [ ("guards", osetD c dstbrGuards)
           , ("mint", multiD dstbrMint)
           , ("script_data_hash", opt c dstbrScriptIntegrityHash)
           , ("aux_hash", opt c dstbrAuxDataHash)
           , ("network", opt c dstbrNetworkId)
           , ("votes", votesD dstbrVotingProcedures)
           , ("proposals", osetD proposalD dstbrProposalProcedures)
           , ("treasury", opt c dstbrCurrentTreasuryValue)
           , ("donation", c dstbrTreasuryDonation)
           , ("top_guards", mapD c (opt dataD) dstbrRequiredTopLevelGuards)
           , ("deposits", mapD c c (unDirectDeposits dstbrDirectDeposits))
           , ("balance_intervals", intervalsD dstbrAccountBalanceIntervals)
           ]

witsD :: TxWits DijkstraEra -> D
witsD w =
  DO
    [ ("vkeys", setD c (w ^. addrTxWitsL))
    , ("boots", setD c (w ^. bootAddrTxWitsL))
    , ("scripts", mapD c scriptD (w ^. scriptTxWitsL))
    , ("datums", mapD c dataD (unTxDats (w ^. datsTxWitsL)))
    , ("redeemers", mapD c (\(d, u) -> DO [("data", dataD d), ("units", c u)]) (unRedeemers (w ^. rdmrsTxWitsL)))
    ]

auxD :: TxAuxData DijkstraEra -> D
auxD a = case getMemoRawType a of
  AlonzoTxAuxDataRaw md ns ps ->
    DO
      [ ("metadata", mapD c c md)
      , ("native", DL (map nativeD (toList ns)))
      , ("plutus", mapD (DS . langName) (DL . map binD . NE.toList) ps)
      ]

subTxD :: Tx SubTx DijkstraEra -> D
subTxD tx =
  DO
    [ ("body", bodyD (tx ^. bodyTxL))
    , ("wits", witsD (tx ^. witsTxL))
    , ("aux", opt auxD (tx ^. auxDataTxL))
    ]

txD :: Tx TopTx DijkstraEra -> D
txD tx =
  DO
    [ ("body", bodyD (tx ^. bodyTxL))
    , ("wits", witsD (tx ^. witsTxL))
    , ("valid", DB (case tx ^. isPhase2ValidTxL of Phase2Valid -> True; Phase2Invalid -> False))
    , ("aux", opt auxD (tx ^. auxDataTxL))
    ]

blockD :: LeiosBlock -> D
blockD (Block h b) =
  let hb = Leios.headerBody h
   in DO
        [ ( "header"
          , DO
              [ ("block_no", c (Leios.hbBlockNo hb))
              , ("slot", c (Leios.hbSlotNo hb))
              , ("prev", c (Leios.hbPrev hb))
              , ("issuer_vk", c (Leios.hbVk hb))
              , ("vrf_vk", c (Leios.hbVrfVk hb))
              , ("vrf", c (Leios.hbVrfRes hb))
              , ("body_size", c (Leios.hbBodySize hb))
              , ("body_hash", c (Leios.hbBodyHash hb))
              , ("ocert", c (Leios.hbOCert hb))
              , ("version", c (Leios.hbVersionInfo hb))
              , ("leios_cert", DB (Leios.hbBlockBodyContainsLeiosCert hb))
              , ("eb", opt c (Leios.hbEbReferencesAnnouncement hb))
              , ("sig", c (encodeFixedSized (Leios.headerSig h)))
              , ("hash", c (Leios.headerHash h))
              ]
          )
        , ("txs", DL (map txD (toList (b ^. txSeqBlockBodyL))))
        , ("leios_cert", opt c (b ^. leiosCertBlockBodyL))
        , ("peras_cert", opt c (b ^. perasCertBlockBodyL))
        ]

-- Decoding --------------------------------------------------------------------

-- | What the oracle reports for one decoded value.
data Report a = Report
  { rFacts :: a -> [(T.Text, Aeson.Value)]
  , rDump :: a -> D
  }

decodeWith ::
  forall a.
  (DecCBOR a, EncCBOR a, Show a) =>
  Report a ->
  BS.ByteString ->
  Aeson.Value
decodeWith r bs = render r (decodeFull' @a ver bs)

-- | Decodes with the annotator instance the node itself uses for this type.
decodeWithAnn ::
  forall a.
  (DecCBOR (Annotator a), EncCBOR a, Show a) =>
  Report a ->
  BS.ByteString ->
  Aeson.Value
decodeWithAnn r bs =
  render r (decodeFullAnnotator @a ver "oracle" decCBOR (BSL.fromStrict bs))

render :: (EncCBOR a, Show a, Show e) => Report a -> Either e a -> Aeson.Value
render r = \case
  Left err -> Aeson.object ["ok" .= False, "err" .= T.pack (take 2000 (show err))]
  Right x ->
    Aeson.object
      [ "ok" .= True
      , "facts" .= Aeson.object [Key.fromText k .= v | (k, v) <- rFacts r x]
      , "dump" .= toJ (rDump r x)
      , "reencode" .= hexT (serialize' ver x)
      , "show" .= T.pack (take 4000 (show x))
      ]

oracleOne :: Kind -> BS.ByteString -> Aeson.Value
oracleOne k bs = case k of
  KTx -> decodeWithAnn @(Tx TopTx DijkstraEra) (Report txFacts txD) bs
  KBody -> decodeWithAnn @(TxBody TopTx DijkstraEra) (Report bodyFacts bodyD) bs
  KWits -> decodeWithAnn @(TxWits DijkstraEra) (Report witsFacts witsD) bs
  KPpu -> decodeWith @(PParamsUpdate DijkstraEra) (Report ppuFacts c) bs
  KBlock -> decodeWithAnn @LeiosBlock (Report blockFacts blockD) bs

runOracle :: Kind -> IO ()
runOracle k = do
  input <- BS.getContents
  mapM_
    ( \line -> do
        let out = case B16.decode (BS8.filter (/= '\r') line) of
              Left e -> Aeson.object ["ok" .= False, "err" .= T.pack ("hex: " <> e)]
              Right bs -> oracleOne k bs
        BSL8.putStrLn (Aeson.encode out)
        hFlush stdout
    )
    (filter (not . BS.null) (BS8.lines input))

main :: IO ()
main =
  getArgs >>= \case
    ["gen", k, source, n, seed] -> runGen (parseKind k) source (read n) (read seed)
    ["oracle", k] -> runOracle (parseKind k)
    _ -> error "usage: conformance-gen gen KIND arb|cddl|cddl-plain N SEED | oracle KIND"
