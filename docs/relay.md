# Running the Posthaste iroh relay

When two paired computers cannot connect directly over QUIC (for example, when
both sit behind strict NATs on different networks), iroh routes their
end-to-end encrypted traffic through a relay server. The relay sees only
device public keys (`EndpointId`) and IP addresses; it never sees note text
or pairing secrets.

These instructions set up `iroh-relay` on a Linux host (such as a mini PC) at
`relay.posthaste.ink`.

## 1. DNS & Firewall

1. Point an `A` (and `AAAA`, if IPv6 is available) DNS record for
   `relay.posthaste.ink` to the server's public IP address.
2. Open the following ports in your router and host firewall:
   - **TCP 80** — ACME HTTP-01 challenge for Let's Encrypt TLS certificates
   - **TCP 443** — HTTPS / WebSocket relay traffic
   - **UDP 7842** (or **UDP 3478**) — QUIC Address Discovery (QAD) for NAT
     hole-punching

## 2. Install `iroh-relay`

Install the relay server binary (or copy a prebuilt release binary from
`n0-computer/iroh` to `/usr/local/bin/iroh-relay`):

```sh
cargo install iroh-relay --features server --locked
sudo cp ~/.cargo/bin/iroh-relay /usr/local/bin/iroh-relay
```

Create a dedicated system user and directories:

```sh
sudo useradd -r -s /usr/sbin/nologin iroh
sudo mkdir -p /etc/iroh /var/lib/iroh-relay
sudo chown -R iroh:iroh /var/lib/iroh-relay
```

## 3. Configure `/etc/iroh/iroh-relay.toml`

```toml
enable_relay = true
http_bind_addr = "[::]:80"

enable_quic_addr_discovery = true
enable_metrics = false
access = "everyone"

[tls]
https_bind_addr = "[::]:443"
quic_bind_addr = "[::]:7842"
cert_dir = "/var/lib/iroh-relay"
cert_mode = "LetsEncrypt"
hostname = "relay.posthaste.ink"
contact = "you@posthaste.ink"
prod_tls = true
```

## 4. Install the systemd unit

Save as `/etc/systemd/system/iroh-relay.service`:

```ini
[Unit]
Description=Posthaste iroh relay
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=iroh
Group=iroh
ExecStart=/usr/local/bin/iroh-relay --config-path /etc/iroh/iroh-relay.toml
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
Restart=on-failure
RestartSec=5s
StateDirectory=iroh-relay
StateDirectoryMode=0750

[Install]
WantedBy=multi-user.target
```

Enable and start it:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now iroh-relay.service
```

## 5. Check that it is up

```sh
sudo systemctl status iroh-relay.service
sudo journalctl -u iroh-relay.service -n 50 --no-pager
curl -I https://relay.posthaste.ink/
```

A healthy relay responds over HTTPS with a valid Let's Encrypt certificate.
