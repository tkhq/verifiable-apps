use qos_core::protocol::services::boot::ManifestEnvelope;
use qos_core::protocol::services::genesis::GenesisMemberOutput;

/// Signed, attested, and audit-friendly output of a resharding run.
///
/// This bundle is what operators fetch after a successful reshard. It ties:
/// - **what ran** (manifest + approvals),
/// - **where/how it ran** (AWS Nitro attestation w/ ephemeral key),
/// - **what it produced** (per-member encrypted shares),
///     together with an **ephemeral-key signature** over the outputs.
///
/// Safe to check into git alongside genesis artifacts.
#[derive(
	Debug,
	Clone,
	PartialEq,
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
