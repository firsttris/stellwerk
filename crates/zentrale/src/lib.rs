//! The Zentrale: stores the configuration, derives each device's desired state, talks to the
//! agents and steers the hub Syncthing.

pub mod agents;
pub mod api;
pub mod app;
pub mod auth;
pub mod desired;
pub mod discovery;
pub mod files;
pub mod hub;
pub mod live;
pub mod model;
pub mod pairing;
pub mod server;
pub mod store;
pub mod waiting;
pub mod web;
