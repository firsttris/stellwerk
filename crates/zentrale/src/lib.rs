//! The Zentrale: stores the configuration, derives each device's desired state, talks to the
//! agents and steers the hub Syncthing.

pub mod agents;
pub mod api;
pub mod app;
pub mod auth;
pub mod desired;
pub mod hub;
pub mod model;
pub mod pairing;
pub mod server;
pub mod store;
pub mod web;
