//! Language features through language servers: Java through SnoutCode's engine (jdtls fed
//! by the Gradle model). The hub runs the servers; the providers answer GPUI Kit's editor;
//! the panels list problems and references.

pub mod edits;
pub mod hierarchy;
pub mod hub;
pub mod java;
pub mod panels;
pub mod providers;
pub mod remote_engine;
pub mod structure;
pub mod symbols;
pub mod text;

pub use hub::{HubEvent, LangHub, hub};
