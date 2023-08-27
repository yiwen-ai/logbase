mod model_log;

pub mod scylladb;

pub use model_log::Log;

pub static MAX_ID: xid::Id = xid::Id([255; 12]);
