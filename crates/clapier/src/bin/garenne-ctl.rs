//! Drive garenne's UDP control port, and listen to its log channel.
//!
//!     garenne-ctl ping
//!     garenne-ctl color 7c5cff
//!     garenne-ctl --ip 192.168.1.42 reboot
//!     garenne-ctl listen

use clap::Parser;
use std::net::IpAddr;

/// Talk to a garenne rabbit over UDP: a command, or `listen` for the
/// log channel.
#[derive(Parser)]
#[command(name = "garenne-ctl", version)]
struct Args {
    /// The rabbit's address
    #[arg(long, default_value = "192.168.1.155", env = "GARENNE_IP")]
    ip: IpAddr,

    /// UDP port of the log channel, for `listen`
    #[arg(long, default_value_t = 9999)]
    log_port: u16,

    /// The command words (ping, reboot, color 7c5cff, ...), or `listen`
    #[arg(required = true)]
    cmd: Vec<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.cmd.len() == 1 && args.cmd[0] == "listen" {
        return listen(args.log_port).await;
    }
    let reply = clapier::ctl_send(args.ip, &args.cmd.join(" "))
        .await
        .map_err(|_| anyhow::anyhow!("no reply (rabbit absent, rebooting, or command lost)"))?;
    println!("{}", reply.trim_end());
    Ok(())
}

/// The log party line: every broadcast datagram, timestamped. The
/// socket joins with SO_REUSEPORT, so a running clapier keeps hearing
/// its pulses too.
async fn listen(port: u16) -> anyhow::Result<()> {
    let sock = clapier::log_socket(port)?;
    eprintln!("listening on UDP :{port} (Ctrl-C to quit)");
    let mut buf = [0u8; 2048];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await?;
        let line = String::from_utf8_lossy(&buf[..n]);
        println!("{} {} {}", wall_clock(), peer.ip(), line.trim_end());
    }
}

/// Local wall-clock as HH:MM:SS.mmm; UTC where localtime is not around.
fn wall_clock() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let millis = now.subsec_millis();
    #[cfg(unix)]
    {
        // libc deprecates the `time_t` alias ahead of musl 1.2 widening it
        // to 64 bits; both the cast and localtime_r track the alias, so this
        // code is correct on either width.
        #[allow(deprecated)]
        let t = now.as_secs() as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        unsafe { libc::localtime_r(&t, &mut tm) };
        format!(
            "{:02}:{:02}:{:02}.{millis:03}",
            tm.tm_hour, tm.tm_min, tm.tm_sec
        )
    }
    #[cfg(not(unix))]
    {
        let s = now.as_secs() % 86_400;
        format!(
            "{:02}:{:02}:{:02}.{millis:03}",
            s / 3600,
            (s / 60) % 60,
            s % 60
        )
    }
}
