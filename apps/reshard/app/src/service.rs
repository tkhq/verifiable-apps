use qos_core::handles;
use qos_core::protocol::services::{
    boot::{ManifestEnvelope, ShareSet},
    genesis::GenesisMemberOutput,
};
use qos_core::protocol::QosHash;
use qos_core::server::RequestProcessor;
use qos_crypto::sha_512;
use qos_nsm::types::{NsmRequest, NsmResponse};
use qos_nsm::NsmProvider;
use qos_p256::P256Public;

use borsh::{from_slice, BorshDeserialize, BorshSerialize};
use prost::Message;

/// Signed, attested, and audit-friendly output of a resharding run.
///
/// This bundle is what operators fetch after a successful reshard. It ties:
/// - **what ran** (manifest + approvals),
/// - **where/how it ran** (AWS Nitro attestation w/ ephemeral key),
/// - **what it produced** (per-member encrypted shares), together with an **ephemeral-key signature** over the outputs.
///
/// Safe to check into git alongside genesis artifacts.
#[derive(
    BorshSerialize,
    BorshDeserialize,
    PartialEq,
    Debug,
    Clone,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ReshardBundle {
    /// Identifies the **exact quorum key** that was resharded.
    ///
    /// Consumers can use this to confirm they're rotating the intended key,
    /// and to cross-reference previous ceremonies for the same key.
    #[serde(with = "qos_hex::serde")]
    pub quorum_public_key: Vec<u8>,

    /// Raw AWS Nitro **attestation document** bytes (as produced by NSM).
    ///
    /// Share holders verify this before posting shares. The document binds:
    /// - the enclave's measured state (PCRs / EIF),
    /// - the **ephemeral public key** created at boot (used below in `signature`),
    /// - the manifest hash via `user_data`.
    #[serde(with = "qos_hex::serde")]
    pub attestation_doc: Vec<u8>,

    /// Envelope that **encapsulates the manifest and its approvals**, including:
    ///
    /// - `manifest`
    /// - `manifest_approvals`
    /// - `share_set_approvals`
    ///
    pub manifest_envelope: ManifestEnvelope,

    /// Per-new-member outputs of the resharding step (**reuses genesis format**).
    ///
    /// Each entry contains:
    /// - the **member’s public key** in the **new share-set**,
    /// - the **encrypted quorum key share** for that member,
    /// - a **share hash** used to validate correct decryption **offline**.
    pub member_outputs: Vec<GenesisMemberOutput>,

    /// Ephemeral-key signature binding outputs to this **attested run**.
    ///
    /// The ephemeral public key is carried in `attestation_doc`. The signature
    /// is computed over `sha512(borsh(member_outputs))`. Verifiers should:
    /// 1) parse & verify the attestation (incl. ephemeral pubkey),
    /// 2) recompute the digest from `member_outputs`,
    /// 3) verify this signature with the ephemeral pubkey.
    #[serde(with = "qos_hex::serde")]
    pub signature: Vec<u8>,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub enum ReshardRequest {
    RetrieveBundle,
    HealthRequest,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub enum ReshardResponse {
    Bundle(Box<ReshardBundle>),
    Error,
    Health,
}

impl ReshardResponse {
    fn error() -> Vec<u8> {
        borsh::to_vec(&Self::Error).expect("serializing should work")
    }
}

pub struct ReshardProcessor {
    cached_reshard_bundle: ReshardBundle,
}

impl ReshardProcessor {
    pub fn new(
        handles: &handles::Handles,
        new_share_set: &ShareSet,
        nsm: &dyn NsmProvider,
    ) -> Result<Self, String> {
        // load keys
        let quorum_pair = handles
            .get_quorum_key()
            .map_err(|e| format!("unable to get quorum key: {e:?}"))?;
        let eph_pair = handles
            .get_ephemeral_key()
            .map_err(|e| format!("unable to get ephemeral key: {e:?}"))?;

        let quorum_pub = quorum_pair.public_key().to_bytes();
        let master_seed = quorum_pair.to_master_seed();

        // get manifest envelope
        let manifest_envelope = handles
            .get_manifest_envelope()
            .map_err(|_| "get_manifest_envelope failed")?;

        // Get attestation doc, which ties the running of this specific instance with:
        // 1. the creation of eph key
        // 2. the manifest and approvals
        let attestation_doc = match nsm.nsm_process_request(NsmRequest::Attestation {
            user_data: Some(manifest_envelope.qos_hash().to_vec()),
            nonce: None,
            public_key: Some(eph_pair.public_key().to_bytes()),
        }) {
            NsmResponse::Attestation { document } => document,
            other => return Err(format!("unexpected NSM response: {other:?}")),
        };

        // Split the master seed
        let n = new_share_set.members.len();
        let k = new_share_set.threshold as usize;
        let shares = qos_crypto::shamir::shares_generate(&master_seed[..], n, k)
            .map_err(|e| format!("shares_generate failed: {e:?}"))?;

        // Encrypt per member of the new share set
        let mut member_outputs = Vec::with_capacity(n);
        for (share, member) in shares.into_iter().zip(new_share_set.members.clone()) {
            let personal_pub = P256Public::from_bytes(&member.pub_key)
                .map_err(|e| format!("bad member pubkey for '{}': {e:?}", member.alias))?;
            let encrypted = personal_pub
                .encrypt(&share)
                .map_err(|e| format!("encryption of share to pub key failed: {e:?}"))?;
            let hash = qos_crypto::sha_512(&share);

            member_outputs.push(GenesisMemberOutput {
                share_set_member: member,
                encrypted_quorum_key_share: encrypted,
                share_hash: hash,
            });
        }

        // borsh serialize the member outputs vector, and sign it with the ephemeral key to tie the running of this specific instance
        // with the creation of these new encrypted shares
        let mo_bytes =
            borsh::to_vec(&member_outputs).map_err(|e| format!("borsh member_outputs: {e}"))?;
        let digest = sha_512(&mo_bytes);
        let signature = eph_pair
            .sign(&digest)
            .map_err(|e| format!("ephemeral sign failed: {e:?}"))?;

        // assemble all outputs together
        let reshard_bundle = ReshardBundle {
            quorum_public_key: quorum_pub,
            attestation_doc,
            manifest_envelope,
            member_outputs,
            signature,
        };

        Ok(Self {
            cached_reshard_bundle: reshard_bundle,
        })
    }
}

impl RequestProcessor for ReshardProcessor {
    fn process(&mut self, request: Vec<u8>) -> Vec<u8> {
        let req: ReshardRequest = match from_slice(&request) {
            Ok(r) => r,
            Err(_) => return ReshardResponse::error(),
        };

        let output = match req {
            ReshardRequest::HealthRequest => ReshardResponse::Health,

            ReshardRequest::RetrieveBundle => {
                ReshardResponse::Bundle(Box::new(self.cached_reshard_bundle.clone()))
            }
        };

        borsh::to_vec(&output).expect("should be valid borsh")
    }
}
