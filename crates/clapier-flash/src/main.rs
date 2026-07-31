//! Flash firmware to a Nabaztag:tag through its bootloader HTTP server.
//!
//! The rabbit must be in config mode (hold the head button while powering
//! on) and this machine joined to the `NabaztagXX` access point it opens.
//!
//! The MTL bootloader runs a tiny HTTP/1.0 server. It accepts a POST to
//! `/c` and scans the raw body for `-violet-` delimiters to find the
//! firmware; it never parses multipart MIME. So the upload is a raw
//! HTTP/1.0 POST with the `.sim` file as the body, no browser involved.
//!
//! The upload is slow by design. The VM stores every TCP segment as a
//! GC-managed heap object in a linked list and walks the whole list on
//! each arrival, so processing is quadratic in segment count and GC
//! pauses grow as the heap fills. The flasher applies backpressure with
//! a tiny send buffer and no send timeout: the rabbit ACKs at its own
//! pace, however long that takes. The connection dropping at the end is
//! the success signal (watchdog reset after the flash).

use anyhow::{Context, Result, bail};
use clap::Parser;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Flash a .sim firmware to a Nabaztag:tag in config mode.
#[derive(Parser)]
#[command(name = "flash-nabaztag", version)]
struct Args {
    /// Path to the .sim firmware file
    firmware: std::path::PathBuf,

    /// The rabbit's address in config mode
    #[arg(long, default_value = "192.168.0.1", env = "NABAZTAG_IP")]
    ip: String,
}

fn log(msg: &str) {
    println!("\x1b[1;34m==>\x1b[0m {msg}");
}

fn ok(msg: &str) {
    println!("\x1b[1;32m OK\x1b[0m {msg}");
}

fn warn(msg: &str) {
    eprintln!("\x1b[1;33mWRN\x1b[0m {msg}");
}

fn progress(sent: usize, total: usize, eta: Option<f64>) {
    let pct = sent * 100 / total;
    let filled = pct / 2;
    let bar: String = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(50 - filled);
    let eta = eta.map(|e| format!(" ETA {e:.0}s")).unwrap_or_default();
    print!("\r    [{bar}] {pct:3}% ({sent}/{total}){eta}   ");
    let _ = std::io::stdout().flush();
}

/// Check the `.sim` structure: `-violet-` head and tail, a hex size
/// field, and a total length that matches it.
fn validate_sim(data: &[u8]) -> Result<()> {
    if data.len() < 24 {
        bail!("file too small ({} bytes) to be a valid .sim", data.len());
    }
    if &data[..8] != b"-violet-" {
        bail!("invalid .sim: does not start with '-violet-'");
    }
    if &data[data.len() - 8..] != b"-violet-" {
        bail!("invalid .sim: does not end with '-violet-'");
    }
    let size_field = std::str::from_utf8(&data[8..16]).context("size field is not ASCII")?;
    let hex_payload_len =
        usize::from_str_radix(size_field, 16).context("invalid size field in .sim")?;
    let expected = 8 + 8 + hex_payload_len + 8;
    if data.len() != expected {
        bail!(
            "size mismatch: file is {} bytes, the size field says {expected}",
            data.len()
        );
    }
    ok(&format!(
        "valid .sim: {} bytes of firmware ({} bytes total)",
        hex_payload_len / 2,
        data.len()
    ));
    Ok(())
}

/// A GET on the config page proves the bootloader server is there.
fn check_connectivity(ip: &str) -> Result<()> {
    log(&format!("checking connectivity to the rabbit at {ip}..."));
    let addr = format!("{ip}:80")
        .parse()
        .context("invalid rabbit address")?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).with_context(
        || format!("cannot reach {ip}: is the rabbit in config mode, and this machine on its access point?"),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(stream, "GET /u.htm HTTP/1.0\r\nHost: {ip}\r\n\r\n")?;
    let mut resp = Vec::new();
    let _ = stream.read_to_end(&mut resp);
    let first_line = resp
        .split(|&b| b == b'\n')
        .next()
        .map(|l| String::from_utf8_lossy(l).trim().to_string())
        .unwrap_or_default();
    if first_line.contains("200") {
        ok("bootloader server responding (GET /u.htm -> 200)");
    } else {
        // The server answered something; that is enough to proceed.
        warn(&format!("GET /u.htm returned: {first_line}"));
    }
    Ok(())
}

/// Give the kernel the same patience the rabbit needs: past 50% the VM
/// can take seconds to ACK a segment, and default retransmission policy
/// drops the connection after roughly a minute of silence.
fn extend_retransmit_patience(stream: &TcpStream) {
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        // TCP_RXT_CONNDROPTIME (0x80): seconds of unacknowledged
        // retransmission before the kernel gives up.
        let secs: libc::c_int = 600;
        let rc = unsafe {
            libc::setsockopt(
                stream.as_raw_fd(),
                libc::IPPROTO_TCP,
                0x80,
                std::ptr::from_ref(&secs).cast(),
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            )
        };
        if rc != 0 {
            warn("could not extend the TCP drop time; the upload may fail if the rabbit is slow");
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        let millis: libc::c_uint = 600_000;
        let rc = unsafe {
            libc::setsockopt(
                stream.as_raw_fd(),
                libc::IPPROTO_TCP,
                libc::TCP_USER_TIMEOUT,
                std::ptr::from_ref(&millis).cast(),
                std::mem::size_of::<libc::c_uint>() as libc::socklen_t,
            )
        };
        if rc != 0 {
            warn("could not extend the TCP drop time; the upload may fail if the rabbit is slow");
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        // Windows retransmission policy is registry-wide; nothing to do
        // per socket. The small send buffer still paces the upload.
        let _ = stream;
    }
}

fn upload(ip: &str, sim: &[u8]) -> Result<()> {
    let header = format!(
        "POST /c HTTP/1.0\r\nHost: {ip}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n",
        sim.len()
    );
    let payload: Vec<u8> = header.as_bytes().iter().chain(sim).copied().collect();
    let total = payload.len();

    log(&format!("uploading firmware to {ip} ({total} bytes)..."));
    let addr = format!("{ip}:80").parse().context("invalid address")?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(10))
        .with_context(|| format!("cannot connect to {ip}:80"))?;
    ok("connected to the rabbit");

    // Nagle stays on (coalescing gives good segment sizes); the small
    // send buffer turns the rabbit's 800-byte receive window into real
    // backpressure on every write.
    socket2::SockRef::from(&stream).set_send_buffer_size(1024)?;
    extend_retransmit_patience(&stream);
    // No timeouts from here on: the rabbit is a 60 MHz ARM7 walking a
    // linked list per segment; it ACKs when it is ready.
    stream.set_write_timeout(None)?;
    stream.set_read_timeout(None)?;

    log("sending; no timeout, the rabbit sets the pace. Expect minutes, not seconds.");
    println!();

    let mut stream = stream;
    let started = Instant::now();
    let mut sent = 0usize;
    for chunk in payload.chunks(512) {
        stream
            .write_all(chunk)
            .with_context(|| format!("connection lost at {sent}/{total} bytes"))?;
        sent += chunk.len();
        let elapsed = started.elapsed().as_secs_f64();
        let eta = (elapsed > 0.0).then(|| (total - sent) as f64 / (sent as f64 / elapsed));
        progress(sent, total, eta);
    }
    println!();
    let elapsed = started.elapsed().as_secs_f64();
    ok(&format!(
        "upload complete: {total} bytes in {elapsed:.1}s ({:.1} KB/s)",
        total as f64 / elapsed / 1024.0
    ));

    log("waiting for the rabbit to decode, decrypt and write the flash...");
    log("the connection dropping here is the success signal (watchdog reset)");
    let mut resp = Vec::new();
    match stream.read_to_end(&mut resp) {
        Ok(_) if resp.is_empty() => ok("connection closed cleanly by the rabbit"),
        Ok(_) => {
            let text = String::from_utf8_lossy(&resp);
            let excerpt: String = text.chars().take(500).collect();
            println!("{excerpt}");
            if text.to_lowercase().contains("error") {
                bail!("the rabbit returned an error; the firmware may be corrupt or incompatible");
            }
            ok("got an HTTP response; the flash may have completed before the reset");
        }
        Err(e) => ok(&format!(
            "connection dropped ({e}), as expected after a flash"
        )),
    }

    println!();
    ok("upload and flash sequence complete");
    log("wait a minute, then power cycle the rabbit; a normal boot means the flash took");
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let sim = std::fs::read(&args.firmware)
        .with_context(|| format!("cannot read {:?}", args.firmware))?;
    log(&format!("firmware : {:?}", args.firmware));
    log(&format!("target   : {}", args.ip));
    println!();
    validate_sim(&sim)?;
    check_connectivity(&args.ip)?;
    println!();
    upload(&args.ip, &sim)
}

#[cfg(test)]
mod tests {
    use super::validate_sim;

    fn fake_sim(payload_hex_len: usize) -> Vec<u8> {
        let mut sim = Vec::new();
        sim.extend_from_slice(b"-violet-");
        sim.extend_from_slice(format!("{payload_hex_len:08x}").as_bytes());
        sim.extend(std::iter::repeat_n(b'a', payload_hex_len));
        sim.extend_from_slice(b"-violet-");
        sim
    }

    #[test]
    fn accepts_a_well_formed_sim() {
        assert!(validate_sim(&fake_sim(64)).is_ok());
    }

    #[test]
    fn rejects_bad_magic_size_and_length() {
        assert!(validate_sim(b"tiny").is_err());
        let mut wrong_magic = fake_sim(64);
        wrong_magic[0] = b'x';
        assert!(validate_sim(&wrong_magic).is_err());
        let mut truncated = fake_sim(64);
        truncated.pop();
        assert!(validate_sim(&truncated).is_err());
    }
}
