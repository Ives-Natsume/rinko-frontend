use rinko_frontend::logging;

#[tokio::main]
async fn main() {
    let _logging_guard = logging::init_logging("logs", "rinko-frontend");
}