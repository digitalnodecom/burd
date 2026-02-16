//! DNS Server for resolving custom TLD domains to localhost
//!
//! This module provides a lightweight DNS server that resolves all queries
//! for the configured TLD to 127.0.0.1, enabling custom local domain names.

use crate::domain::DEFAULT_DNS_PORT;
use hickory_proto::op::{MessageType, OpCode, ResponseCode};
use hickory_proto::rr::{DNSClass, RData, Record, RecordType};
use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// DNS Server state
pub struct DnsServer {
    port: u16,
    tld: String,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl DnsServer {
    pub fn new(port: u16, tld: String) -> Self {
        Self {
            port,
            tld,
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    /// Start the DNS server in a background thread
    pub fn start(&mut self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(()); // Already running
        }

        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let socket = UdpSocket::bind(addr)
            .map_err(|e| format!("Failed to bind DNS server to {}: {}", addr, e))?;

        // Set socket timeout so we can check the running flag
        socket.set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| format!("Failed to set socket timeout: {}", e))?;

        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let tld = self.tld.clone();

        let handle = thread::spawn(move || {
            let mut buf = [0u8; 512];

            while running.load(Ordering::SeqCst) {
                match socket.recv_from(&mut buf) {
                    Ok((len, src)) => {
                        if let Some(response) = handle_dns_query(&buf[..len], &tld) {
                            let _ = socket.send_to(&response, src);
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // Timeout, continue to check running flag
                        continue;
                    }
                    Err(_) => {}
                }
            }
        });

        self.handle = Some(handle);
        Ok(())
    }

    /// Get the TLD this server is configured for
    #[allow(dead_code)]
    pub fn tld(&self) -> &str {
        &self.tld
    }

    /// Stop the DNS server
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if the DNS server is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the port the DNS server is running on
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Default for DnsServer {
    fn default() -> Self {
        Self::new(DEFAULT_DNS_PORT, crate::domain::DEFAULT_TLD.to_string())
    }
}

impl Drop for DnsServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Handle a DNS query and return a response
fn handle_dns_query(query_data: &[u8], tld: &str) -> Option<Vec<u8>> {
    use hickory_proto::op::Message;

    // Parse the incoming query
    let query = Message::from_bytes(query_data).ok()?;

    // Only handle standard queries
    if query.op_code() != OpCode::Query {
        return None;
    }

    // Build response header
    let mut response = Message::new();
    response.set_id(query.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(OpCode::Query);
    response.set_authoritative(true);
    response.set_recursion_desired(query.recursion_desired());
    response.set_recursion_available(false);
    response.set_response_code(ResponseCode::NoError);

    // Copy queries to response
    for query_record in query.queries() {
        response.add_query(query_record.clone());

        let name = query_record.name();
        let name_str = name.to_string().to_lowercase();

        // Check if this is a query for our TLD
        let tld_suffix = format!(".{}.", tld);
        let is_our_tld = name_str.ends_with(&tld_suffix) ||
                         name_str == format!("{}.", tld);

        if is_our_tld && query_record.query_type() == RecordType::A {
            // Create A record pointing to localhost
            let mut record = Record::new();
            record.set_name(name.clone());
            record.set_rr_type(RecordType::A);
            record.set_dns_class(DNSClass::IN);
            record.set_ttl(300); // 5 minute TTL
            record.set_data(Some(RData::A(hickory_proto::rr::rdata::A(Ipv4Addr::new(127, 0, 0, 1)))));

            response.add_answer(record);
        } else if !is_our_tld {
            // Not our TLD, return NXDOMAIN
            response.set_response_code(ResponseCode::NXDomain);
        }
    }

    // Encode response
    response.to_bytes().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_server_lifecycle() {
        let mut server = DnsServer::new(15354, "test".to_string()); // Use high port for testing

        assert!(!server.is_running());

        // Start might fail if port is in use, that's ok for this test
        if server.start().is_ok() {
            assert!(server.is_running());
            server.stop();
            assert!(!server.is_running());
        }
    }
}
