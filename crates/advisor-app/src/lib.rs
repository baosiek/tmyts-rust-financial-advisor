//! The composition root: the one place that turns the settings file into a
//! working agent. Every binary (the CLI today, the evals runner, the Axum API
//! later) builds its agent here, so they all run exactly the same setup.

mod app;
pub mod config;
pub mod providers;
pub mod startup;
pub mod tools;

pub use app::{App, AppError};
