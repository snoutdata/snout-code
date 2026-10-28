//! Language features through language servers: Java through SnoutCode's engine (jdtls fed
//! by the Gradle model). The hub runs the servers; the providers answer GPUI Kit's editor;
//! the panels list problems and references.

pub mod edits;
pub mod hub;
pub mod java;
pub mod panels;
pub mod providers;
pub mod text;

pub use hub::{HubEvent, LangHub, hub};
