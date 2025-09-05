//! CLI for reshard app.

use qos_core::{
    cli::{EPHEMERAL_FILE_OPT, MANIFEST_FILE_OPT, QUORUM_FILE_OPT, USOCK},
    handles::Handles,
    io::SocketAddress,
    parser::{GetParserForOptions, OptionsParser, Parser, Token},
    protocol::services::boot::ShareSet,
    server::SocketServer,
    EPHEMERAL_KEY_FILE, MANIFEST_FILE, QUORUM_FILE, SEC_APP_SOCK,
};

/// CLI options for starting up the app server.
#[derive(Default, Clone, Debug, PartialEq)]
struct ReshardOpts {
    parsed: Parser,
}

const MOCK_NSM: &str = "mock-nsm";
const NEW_SHARE_SET: &str = "new-share-set";

impl ReshardOpts {
    fn new(args: &mut Vec<String>) -> Self {
        let parsed = OptionsParser::<ReshardParser>::parse(args)
            .expect("provided invalid CLI args for Reshard app");

        Self { parsed }
    }

    fn addr(&self) -> SocketAddress {
        SocketAddress::new_unix(self.parsed.single(USOCK).expect("unix socket is required"))
    }

    /// Defaults to [`QUORUM_FILE`] if not explicitly specified
    fn quorum_file(&self) -> String {
        self.parsed
            .single(QUORUM_FILE_OPT)
            .expect("has a default value.")
            .clone()
    }

    /// Defaults to [`EPHEMERAL_KEY_FILE`] if not explicitly specified
    fn ephemeral_file(&self) -> String {
        self.parsed
            .single(EPHEMERAL_FILE_OPT)
            .expect("has a default value.")
            .clone()
    }

    /// Defaults to [`MANIFEST_FILE`] if not explicitly specified
    fn manifest_file(&self) -> String {
        self.parsed
            .single(MANIFEST_FILE_OPT)
            .expect("has a default value.")
            .clone()
    }

    fn mock_nsm(&self) -> bool {
        self.parsed.flag(MOCK_NSM).unwrap_or(false)
    }

    // Return a parsed ShareSet
    fn share_set(&self) -> ShareSet {
        let arg = self
            .parsed
            .single(NEW_SHARE_SET)
            .expect("--new-share-set is required");

        serde_json::from_str(arg).expect("share set is not valid json")
    }
}

struct ReshardParser;
impl GetParserForOptions for ReshardParser {
    fn parser() -> Parser {
        Parser::new()
            .token(
                Token::new(USOCK, "unix socket (`.sock`) to listen on.")
                    .takes_value(true)
                    .forbids(vec!["port", "cid"])
                    .default_value(SEC_APP_SOCK),
            )
			.token(
				Token::new(QUORUM_FILE_OPT, "path to file where the Quorum Key secret should be stored. Use default for production.")
					.takes_value(true)
					.default_value(QUORUM_FILE)
			)
			.token(
				Token::new(EPHEMERAL_FILE_OPT, "path to file where the Ephemeral Key secret should be stoored. Use default for production.")
					.takes_value(true)
					.default_value(EPHEMERAL_KEY_FILE)
			)
			.token(
				Token::new(MANIFEST_FILE_OPT, "path to file where the Manifest should be stored. Use default for production")
					.takes_value(true)
					.default_value(MANIFEST_FILE)
			)
            .token(
                Token::new(
                    NEW_SHARE_SET,
                    r#"JSON ShareSet. Pass JSON inline, or "-" to read from stdin.
							Example:
							{"threshold":3,"members":[{"alias":"reshard-1","pubKey":"04..."}]}"#,
                )
                .takes_value(true),
            )
            .token(Token::new(
                MOCK_NSM,
                "use the MockNsm. Should never be used in production",
            ))
    }
}

/// Reshard CLI.
pub struct Cli;
impl Cli {
    /// Execute the CLI.
    ///
    /// # Panics
    ///
    /// Panics if the socket server errors.
    pub fn execute() {
        let mut args: Vec<String> = std::env::args().collect();

        let opts = ReshardOpts::new(&mut args);

        if opts.parsed.version() {
            println!("version: {}", env!("CARGO_PKG_VERSION"));
        } else if opts.parsed.help() {
            println!("{}", opts.parsed.info());
        } else {
            let nsm: Box<dyn qos_nsm::NsmProvider> = if opts.mock_nsm() {
                #[cfg(feature = "vsock")]
                panic!("cannot use mock nsm when \"vsock\" feature is enabled");
                #[cfg(all(not(feature = "vsock"), feature = "mock"))]
                {
                    Box::new(qos_nsm::mock::MockNsm)
                }
                #[cfg(all(not(feature = "vsock"), not(feature = "mock")))]
                panic!("cannot use mock nsm when \"mock\" feature is not enabled");
            } else {
                Box::new(qos_nsm::Nsm)
            };

            // Build processor; panic on error so the app fails to come up if anything is wrong
            let processor = crate::service::ReshardProcessor::new(
                &Handles::new(
                    opts.ephemeral_file(),
                    opts.quorum_file(),
                    opts.manifest_file(),
                    "pivot not used".to_string(),
                ),
                &opts.share_set(),
                nsm.as_ref(),
            )
            .unwrap_or_else(|e| panic!("reshard precompute failed: {e}"));

            println!("---- Starting Reshard server -----");
            SocketServer::listen(opts.addr(), processor).expect("unable to start Reshard server");
        }
    }
}
