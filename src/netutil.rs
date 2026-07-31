//! 本机局域网地址探测

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

fn is_usable_lan_v4(ip: Ipv4Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_link_local() || ip.is_multicast() {
        return false;
    }
    // Clash / Mihomo TUN 常用 198.18.0.0/15，不是真实局域网
    let o = ip.octets();
    if o[0] == 198 && (o[1] == 18 || o[1] == 19) {
        return false;
    }
    // 优先 RFC1918 私网
    ip.is_private()
}

fn collect_from_ip_cmd() -> Vec<Ipv4Addr> {
    let Ok(output) = std::process::Command::new("ip")
        .args(["-4", "-br", "addr", "show"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut ips = Vec::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        // 跳过虚拟/容器网卡
        if lower.starts_with("lo ")
            || lower.contains("docker")
            || lower.contains("veth")
            || lower.contains("br-")
            || lower.contains("virbr")
            || lower.contains("tun")
            || lower.contains("mihomo")
            || lower.contains("meta")
            || lower.contains("wg")
        {
            continue;
        }
        // 只要 UP 的物理/局域网口
        if !lower.contains(" up ") && !line.contains(" UP ") {
            continue;
        }
        for token in line.split_whitespace().skip(2) {
            let addr = token.split('/').next().unwrap_or("");
            if let Ok(ip) = addr.parse::<Ipv4Addr>() {
                if is_usable_lan_v4(ip) && !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
        }
    }
    ips
}

fn collect_from_udp() -> Vec<Ipv4Addr> {
    let mut ips = Vec::new();
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        // 连到私网网关探测（比 8.8.8.8 更不易落到 TUN）
        for dest in ["192.168.0.1:80", "10.0.0.1:80", "172.16.0.1:80"] {
            if sock.connect(dest).is_err() {
                continue;
            }
            if let Ok(addr) = sock.local_addr() {
                if let IpAddr::V4(ip) = addr.ip() {
                    if is_usable_lan_v4(ip) && !ips.contains(&ip) {
                        ips.push(ip);
                    }
                }
            }
        }
    }
    ips
}

/// 返回可用于局域网访问的 base URL，如 `http://192.168.1.10:18765`
pub fn lan_base_urls(port: u16) -> Vec<String> {
    let mut ips = collect_from_ip_cmd();
    for ip in collect_from_udp() {
        if !ips.contains(&ip) {
            ips.push(ip);
        }
    }

    // 192.168 优先排前面
    ips.sort_by_key(|ip| {
        let o = ip.octets();
        if o[0] == 192 && o[1] == 168 {
            0u8
        } else if o[0] == 10 {
            1
        } else {
            2
        }
    });

    if ips.is_empty() {
        return vec![format!("http://127.0.0.1:{port}")];
    }

    ips.into_iter()
        .map(|ip| format!("http://{ip}:{port}"))
        .collect()
}
