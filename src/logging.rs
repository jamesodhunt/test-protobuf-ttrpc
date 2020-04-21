use slog::o;
use slog::Drain;
use slog_async;
use slog_json;
use std::io::Write;
use std::process;

// Convenience macro to obtain the scope logger
#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

pub fn create_logger<W>(source: &str, writer: W) -> slog::Logger
where
    W: Write + Send + Sync + 'static,
{
    let json_drain = slog_json::Json::new(writer)
        .add_default_keys()
        .build()
        .fuse();

    let async_drain = slog_async::Async::default(json_drain);

    slog::Logger::root(
        async_drain.fuse(),
        o!("pid" => process::id().to_string(),
            "source" => source.to_string()),
    )
}
