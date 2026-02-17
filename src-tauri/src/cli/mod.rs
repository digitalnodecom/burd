//! CLI module for the burd command-line tool
//!
//! Provides commands for managing Burd instances from the terminal.

pub mod analyze;
pub mod db;
pub mod doctor;
pub mod env;
pub mod init;
pub mod link;
pub mod mcp;
pub mod mysql;
pub mod new;
pub mod open;
pub mod park;
pub mod postgres;
pub mod proxy;
pub mod secure;
pub mod setup;
pub mod share;
pub mod upgrade;

pub use analyze::run_analyze;
pub use db::{run_db_create, run_db_drop, run_db_export, run_db_import, run_db_list, run_db_shell};
pub use doctor::run_doctor;
pub use env::{run_env_check, run_env_fix, run_env_show};
pub use init::run_init;
pub use link::{run_link, run_links, run_unlink};
pub use mcp::run_mcp;
pub use mysql::{list_mysql_tools, run_mysql};
pub use new::run_new;
pub use open::run_open;
pub use park::{run_forget, run_park, run_parked, run_refresh, run_status};
pub use postgres::{list_postgres_tools, run_postgres};
pub use proxy::{run_proxies, run_proxy, run_unproxy};
pub use secure::{run_secure, run_unsecure};
pub use setup::run_setup;
pub use share::run_share;
pub use upgrade::run_upgrade;
