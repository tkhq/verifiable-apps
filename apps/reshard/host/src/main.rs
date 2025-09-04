use reshard_host::cli::CLI;

#[tokio::main]
async fn main() {
    CLI::execute().await;
}
