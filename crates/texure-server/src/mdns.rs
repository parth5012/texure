// mDNS LAN discovery (map #36, ticket #32).
//
// Hand-rolled DNS-SD over existing tokio UDP per the map
// zero-external-dependency decision (no mdns-sd crate). Pure packet
// build/parse functions are unit-testable without network access.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tracing::warn;

/// mDNS service type advertised by the bridge (`NsdManager`, BridgeService).
pub const SERVICE_TYPE: &str = "_texure._tcp.local.";
/// Fixed bridge HTTP port (bridge-contract: port is FIXED 8765).
pub const BRIDGE_PORT: u16 = 8765;
/// Bridge TXT version advertised to resolvers.
pub const VERSION: &str = "0.1.0";
/// IPv4 mDNS multicast group:port.
pub const MULTICAST: &str = "224.0.0.251:5353";
/// Default discovery wait when `url="auto"` (ticket: ~2 s).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

const QTYPE_PTR: u16 = 12;
const QTYPE_TXT: u16 = 16;
const QTYPE_SRV: u16 = 33;
const QTYPE_A: u16 = 1;

/// A discovered Texure bridge instance on the LAN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredService {
    /// DNS-SD instance name, e.g. `Texure-com.texure.sample._texure._tcp.local.`
    pub instance: String,
    /// Target host from SRV (or its A record IP when present).
    pub host: String,
    pub port: u16,
    pub version: Option<String>,
    pub app_id: Option<String>,
}

impl DiscoveredService {
    /// Resolved HTTP base URL for the bridge client.
    pub fn resolved_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Whether a resolved config URL requests auto-discovery (`url="auto"`).
/// Omitted/empty config URLs keep existing semantics (`NO_CONFIG`); only
/// the explicit `"auto"` value triggers discovery.
pub fn is_auto_url(url: &str) -> bool {
    url.trim() == "auto"
}

/// Encode a DNS name (`_texure._tcp.local.`) into wire-format labels.
pub fn encode_dns_name(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for label in name.split('.') {
        if label.is_empty() {
            continue;
        }
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    out
}

/// Decode a DNS name at `offset`, following compression pointers.
/// Returns the name with a trailing dot (wire round-trip form) and the
/// offset just past the name in the original stream.
fn decode_dns_name(packet: &[u8], offset: usize) -> Option<(String, usize)> {
    let mut labels: Vec<String> = Vec::new();
    let mut pos = offset;
    let mut next: Option<usize> = None;
    let mut jumps = 0;
    loop {
        if pos >= packet.len() {
            return None;
        }
        let len = packet[pos];
        if len == 0 {
            pos += 1;
            break;
        }
        if len & 0xC0 == 0xC0 {
            if pos + 1 >= packet.len() {
                return None;
            }
            let ptr = ((len as usize & 0x3F) << 8) | packet[pos + 1] as usize;
            if next.is_none() {
                next = Some(pos + 2);
            }
            jumps += 1;
            if jumps > 8 {
                return None;
            }
            pos = ptr;
            continue;
        }
        if len & 0xC0 != 0 {
            return None;
        }
        pos += 1;
        if pos + len as usize > packet.len() {
            return None;
        }
        labels.push(String::from_utf8_lossy(&packet[pos..pos + len as usize]).into_owned());
        pos += len as usize;
    }
    if labels.is_empty() {
        return Some((String::new(), next.unwrap_or(pos)));
    }
    Some((format!("{}.", labels.join(".")), next.unwrap_or(pos)))
}

/// Build a DNS-SD PTR query for `service_type` (QTYPE PTR, QCLASS IN).
pub fn build_ptr_query(service_type: &str) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(64);
    pkt.extend_from_slice(&0u16.to_be_bytes()); // id
    pkt.extend_from_slice(&0x0100u16.to_be_bytes()); // RD flag
    pkt.extend_from_slice(&1u16.to_be_bytes()); // qd
    pkt.extend_from_slice(&0u16.to_be_bytes()); // an
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ns
    pkt.extend_from_slice(&0u16.to_be_bytes()); // ar
    pkt.extend_from_slice(&encode_dns_name(service_type));
    pkt.extend_from_slice(&QTYPE_PTR.to_be_bytes());
    pkt.extend_from_slice(&1u16.to_be_bytes()); // IN
    pkt
}

/// Parse an mDNS response packet into discovered Texure services.
/// Tolerates truncated/malformed packets (returns what parsed cleanly,
/// never panics). Only instances linked by a `_texure._tcp.local.` PTR
/// record are returned; A records substitute the host IP when present.
pub fn parse_mdns_response(packet: &[u8]) -> Vec<DiscoveredService> {
    if packet.len() < 12 {
        return Vec::new();
    }
    let rd16 = |o: usize| -> u16 { u16::from_be_bytes([packet[o], packet[o + 1]]) };
    let qd = rd16(4) as usize;
    let rr_count = (rd16(6) as usize) + (rd16(8) as usize) + (rd16(10) as usize);

    let mut offset = 12;
    for _ in 0..qd {
        let Some((_, next)) = decode_dns_name(packet, offset) else {
            return Vec::new();
        };
        if next + 4 > packet.len() {
            return Vec::new();
        }
        offset = next + 4;
    }

    // (service_type, instance) via PTR; (instance -> (host, port)) via SRV;
    // (instance -> txt) ; (host -> ip) via A.
    let mut instances: Vec<String> = Vec::new();
    let mut srv: Vec<(String, String, u16)> = Vec::new();
    let mut txt: HashMap<String, (Option<String>, Option<String>)> = HashMap::new();
    let mut a_records: HashMap<String, String> = HashMap::new();

    for _ in 0..rr_count {
        let Some((owner, next)) = decode_dns_name(packet, offset) else {
            break;
        };
        if next + 10 > packet.len() {
            break;
        }
        let rtype = rd16(next);
        let rdlen = rd16(next + 8) as usize;
        let rdata_start = next + 10;
        if rdata_start + rdlen > packet.len() {
            break;
        }
        match rtype {
            QTYPE_PTR => {
                if let Some((target, _)) = decode_dns_name(packet, rdata_start) {
                    if owner.eq_ignore_ascii_case(SERVICE_TYPE) {
                        instances.push(target);
                    }
                }
            }
            QTYPE_SRV => {
                if rdlen >= 6 {
                    let port =
                        u16::from_be_bytes([packet[rdata_start + 4], packet[rdata_start + 5]]);
                    if let Some((host, _)) = decode_dns_name(packet, rdata_start + 6) {
                        srv.push((owner.clone(), host, port));
                    }
                }
            }
            QTYPE_TXT => {
                let mut pos = rdata_start;
                let end = rdata_start + rdlen;
                let mut version = None;
                let mut app_id = None;
                while pos < end {
                    let len = packet[pos] as usize;
                    pos += 1;
                    if pos + len > end {
                        break;
                    }
                    let s = String::from_utf8_lossy(&packet[pos..pos + len]).into_owned();
                    if let Some((k, v)) = s.split_once('=') {
                        match k {
                            "version" => version = Some(v.to_string()),
                            "appId" => app_id = Some(v.to_string()),
                            _ => {}
                        }
                    }
                    pos += len;
                }
                txt.insert(owner.clone(), (version, app_id));
            }
            QTYPE_A if rdlen == 4 => {
                let ip = IpAddr::V4(Ipv4Addr::new(
                    packet[rdata_start],
                    packet[rdata_start + 1],
                    packet[rdata_start + 2],
                    packet[rdata_start + 3],
                ))
                .to_string();
                a_records.insert(owner.clone(), ip);
            }
            _ => {}
        }
        offset = rdata_start + rdlen;
    }

    srv.into_iter()
        .filter(|(instance, _, _)| instances.iter().any(|i| i == instance))
        .map(|(instance, host, port)| {
            let addr = a_records.get(&host).cloned();
            let (version, app_id) = txt.get(&instance).cloned().unwrap_or((None, None));
            // Prefer the routable IP over the `.local.` hostname when the
            // responder supplied an A record (Windows clients cannot resolve
            // `.local.` hostnames via mDNS without a responder).
            let host = addr.unwrap_or(host);
            DiscoveredService {
                instance,
                host,
                port,
                version,
                app_id,
            }
        })
        .collect()
}

/// Discover a LAN Texure bridge via mDNS within `timeout`.
/// Best-effort multicast join: if the group join fails (Wi-Fi isolation,
/// no multicast route) the unicast listener still gets a chance, and a
/// timeout simply yields `None` so callers apply `OFFLINE` semantics.
pub async fn discover_with_timeout(timeout: Duration) -> Option<DiscoveredService> {
    let socket = UdpSocket::bind("0.0.0.0:0").await.ok()?;
    let multicast: SocketAddr = MULTICAST.parse().ok()?;
    let _ = socket.join_multicast_v4(Ipv4Addr::new(224, 0, 0, 251), Ipv4Addr::UNSPECIFIED);
    let query = build_ptr_query(SERVICE_TYPE);
    if socket.send_to(&query, multicast).await.is_err() {
        return None;
    }

    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 4096];
    loop {
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        let remaining = deadline - now;
        match tokio::time::timeout(remaining, socket.recv(&mut buf)).await {
            Ok(Ok(len)) => {
                let services = parse_mdns_response(&buf[..len]);
                if let Some(first) = services.into_iter().next() {
                    return Some(first);
                }
            }
            Ok(Err(e)) => {
                warn!("mDNS discovery recv failed: {e}");
                return None;
            }
            Err(_) => return None,
        }
    }
}

/// Discover a LAN Texure bridge within [`DEFAULT_TIMEOUT`].
pub async fn discover() -> Option<DiscoveredService> {
    discover_with_timeout(DEFAULT_TIMEOUT).await
}

#[cfg(test)]
mod tests {
    use super::{
        build_ptr_query, encode_dns_name, is_auto_url, parse_mdns_response, DiscoveredService,
        BRIDGE_PORT, SERVICE_TYPE, VERSION,
    };

    // Local packet builders for tests (mirror of DNS wire format).
    fn push_name(out: &mut Vec<u8>, name: &str) {
        out.extend_from_slice(&encode_dns_name(name));
    }

    fn push_rr(out: &mut Vec<u8>, name: &str, rtype: u16, rdata: &[u8]) {
        push_name(out, name);
        out.extend_from_slice(&rtype.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes()); // IN
        out.extend_from_slice(&120u32.to_be_bytes()); // TTL
        out.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        out.extend_from_slice(rdata);
    }

    fn build_response() -> Vec<u8> {
        let instance = "Texure-com.texure.sample._texure._tcp.local.";
        let host = "phone-1.local.";
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&0u16.to_be_bytes()); // id
        pkt.extend_from_slice(&0x8400u16.to_be_bytes()); // response, authoritative
        pkt.extend_from_slice(&0u16.to_be_bytes()); // qd
        pkt.extend_from_slice(&1u16.to_be_bytes()); // an
        pkt.extend_from_slice(&0u16.to_be_bytes()); // ns
        pkt.extend_from_slice(&3u16.to_be_bytes()); // ar

        // PTR answer: service -> instance
        let mut rdata = Vec::new();
        push_name(&mut rdata, instance);
        push_rr(&mut pkt, SERVICE_TYPE, 12, &rdata);

        // SRV additional: instance -> host:8765
        let mut srv = Vec::new();
        srv.extend_from_slice(&0u16.to_be_bytes()); // priority
        srv.extend_from_slice(&0u16.to_be_bytes()); // weight
        srv.extend_from_slice(&BRIDGE_PORT.to_be_bytes());
        push_name(&mut srv, host);
        push_rr(&mut pkt, instance, 33, &srv);

        // TXT additional: version + appId
        let mut txt = Vec::new();
        for entry in [
            format!("version={VERSION}"),
            "appId=com.texure.sample".to_string(),
        ] {
            txt.push(entry.len() as u8);
            txt.extend_from_slice(entry.as_bytes());
        }
        push_rr(&mut pkt, instance, 16, &txt);

        pkt
    }

    #[test]
    fn test_ptr_query_targets_texure_service() {
        let pkt = build_ptr_query(SERVICE_TYPE);
        // Header: id(2) flags(2) qd(2) an(2) ns(2) ar(2)
        assert_eq!(&pkt[4..6], &1u16.to_be_bytes());
        // QNAME present, QTYPE=PTR(12), QCLASS=IN(1) at tail
        let tail = &pkt[pkt.len() - 4..];
        assert_eq!(tail, &[0u8, 12, 0, 1]);
        let decoded = &pkt[12..pkt.len() - 4];
        assert_eq!(decoded, &encode_dns_name(SERVICE_TYPE));
    }

    #[test]
    fn test_is_auto_url() {
        assert!(is_auto_url("auto"));
        assert!(is_auto_url("  auto  "));
        assert!(!is_auto_url("http://192.168.1.1:8765"));
        assert!(!is_auto_url(""));
    }

    #[test]
    fn test_parse_response_yields_discovered_service() {
        let pkt = build_response();
        let services = parse_mdns_response(&pkt);
        assert_eq!(services.len(), 1);
        let svc = &services[0];
        assert_eq!(svc.port, 8765);
        assert_eq!(svc.host, "phone-1.local.");
        assert_eq!(svc.version.as_deref(), Some("0.1.0"));
        assert_eq!(svc.app_id.as_deref(), Some("com.texure.sample"));
        assert_eq!(svc.resolved_url(), "http://phone-1.local.:8765");
    }

    #[test]
    fn test_parse_truncated_packet_is_empty() {
        assert!(parse_mdns_response(&[0u8, 1, 2]).is_empty());
        assert!(parse_mdns_response(&[]).is_empty());
    }

    #[test]
    fn test_parse_ignores_other_services() {
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&0u16.to_be_bytes());
        pkt.extend_from_slice(&0x8400u16.to_be_bytes());
        pkt.extend_from_slice(&0u16.to_be_bytes());
        pkt.extend_from_slice(&1u16.to_be_bytes());
        pkt.extend_from_slice(&0u16.to_be_bytes());
        pkt.extend_from_slice(&0u16.to_be_bytes());
        let mut rdata = Vec::new();
        push_name(&mut rdata, "Foo._other._tcp.local.");
        push_rr(&mut pkt, "_other._tcp.local.", 12, &rdata);
        assert!(parse_mdns_response(&pkt).is_empty());
    }

    #[test]
    fn test_discovered_service_type_is_stable() {
        let svc = DiscoveredService {
            instance: "x".to_string(),
            host: "h.local.".to_string(),
            port: BRIDGE_PORT,
            version: None,
            app_id: None,
        };
        assert_eq!(SERVICE_TYPE, "_texure._tcp.local.");
        assert_eq!(svc.resolved_url(), "http://h.local.:8765");
    }
}
