//! Integration test for reshard app
use reshard_app::service::ReshardBundle;
use std::path::PathBuf;

use reshard_host::generated::reshard::reshard_service_client::ReshardServiceClient;
use reshard_host::generated::reshard::RetrieveReshardRequest;

use e2e::TestArgs;
use qos_p256::{P256Pair, P256Public};

#[tokio::test]
async fn reshard_e2e_json() {
    async fn test(args: TestArgs) {
        let mut client: ReshardServiceClient<_> = args.reshard_client;

        let resp = client
            .retrieve_reshard(tonic::Request::new(RetrieveReshardRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert!(
            !resp.reshard_bundle.is_empty(),
            "server returned empty JSON"
        );

        // Make sure we can rehydrate the bundle
        let bundle: ReshardBundle = serde_json::from_str(&resp.reshard_bundle).expect("valid JSON");

        // Decrypt each member's share using the fixture private keys
        let secrets_dir = PathBuf::from("./fixtures/reshard/new-share-set-secrets");
        let mut shares: Vec<Vec<u8>> = Vec::with_capacity(bundle.member_outputs.len());
        for m in bundle.member_outputs.iter() {
            let alias = m.share_set_member.alias.clone();
            let sk_path = secrets_dir.join(format!("{alias}.secret"));
            let pair = P256Pair::from_hex_file(sk_path.to_str().unwrap())
                .expect("load member private key");
            let pt = pair
                .decrypt(&m.encrypted_quorum_key_share)
                .expect("decrypt share");

            // integrity: verify hash matches
            assert_eq!(
                qos_crypto::sha_512(&pt),
                m.share_hash,
                "share hash mismatch for {alias}",
            );

            shares.push(pt);
        }

        let quorum_secret_path = "./fixtures/reshard/quorum.secret";
        let expected_pair =
            qos_p256::P256Pair::from_hex_file(quorum_secret_path).expect("load quorum.secret");
        let expected_pub = expected_pair.public_key().to_bytes();
        let k = std::fs::read_to_string("./fixtures/reshard/new-share-set/quorum_threshold")
            .expect("read threshold");
        let k: usize = k.trim().parse::<usize>().expect("parse threshold");

        // Positive check: ALL k-of-n combos must reconstruct the quorum key
        for combo in qos_crypto::n_choose_k::combinations(&shares, k) {
            let seed_vec = qos_crypto::shamir::shares_reconstruct(&combo).unwrap();

            let seed: [u8; 32] = seed_vec
                .as_slice()
                .try_into()
                .expect("reconstructed seed must be 32 bytes");

            let quorum_key = P256Pair::from_master_seed(&seed).unwrap();

            assert_eq!(
                quorum_key.public_key().to_bytes(),
                expected_pub,
                "quorum key public mismatch",
            );
        }

        // Negative checks: for every r < k, NO combo should yield the quorum pubkey
        for r in 1..k {
            let mut matches = 0usize;
            let mut errs = 0usize;
            let mut mismatches = 0usize;

            for combo in qos_crypto::n_choose_k::combinations(&shares, r) {
                match qos_crypto::shamir::shares_reconstruct(&combo) {
                    Err(_e) => {
                        errs += 1;
                    }
                    Ok(seed_vec) => {
                        // Even if the lib returns something, it must NOT match the real key
                        if let Ok(seed) = <[u8; 32]>::try_from(seed_vec.as_slice()) {
                            let qp = P256Pair::from_master_seed(&seed).unwrap();
                            if qp.public_key().to_bytes() == expected_pub {
                                matches += 1; // this would be a failure
                            } else {
                                mismatches += 1;
                            }
                        } else {
                            // Wrong length => cannot match
                            mismatches += 1;
                        }
                    }
                }
            }
            println!("r={r}: reconstruct_errs={errs}, non-matching_reconstructions={mismatches}, matches={matches}");

            // Assert we never matched with fewer than k shares.
            assert_eq!(
                matches, 0,
                "found an unexpected quorum key match using only {r} shares (< {k})"
            );
        }

        // Verify the signature over the member output was by the ephemeral key
        // bytes we signed: borsh(member_outputs)
        let mo_bytes = borsh::to_vec(&bundle.member_outputs).expect("borsh");
        let digest = qos_crypto::sha_512(&mo_bytes);

        // verify signature
        let eph_pub = P256Public::from_hex_file("./fixtures/reshard/ephemeral.pub")
            .expect("load ephemeral.pub");

        eph_pub
            .verify(&digest, &bundle.signature)
            .expect("ephemeral sig verify");

        // Sanity check random pub key doesn't verify
        let random_key = P256Pair::generate().unwrap();
        let random_key_pub = random_key.public_key();

        let res = random_key_pub.verify(&digest, &bundle.signature);
        assert!(
            res.is_err(),
            "verification unexpectedly succeeded with random key"
        );
    }
    e2e::execute(test).await;
}
