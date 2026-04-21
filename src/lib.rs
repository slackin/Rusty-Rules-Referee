// Rusty Rules Referee (R3) — Game server administration bot written in Rust
// Originally inspired by Big Brother Bot by Michael "ThorN" Thornton
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.

pub mod config;
pub mod core;
pub mod events;
pub mod parsers;
pub mod plugins;
pub mod rcon;
pub mod storage;
pub mod sync;
pub mod update;
pub mod web;
pub mod maprepo;
pub mod mapscan;

/// Create a TCP listener with `SO_REUSEADDR` set, so a restart doesn't fail
/// with "Address already in use" while the old socket is in TIME_WAIT.
pub fn bind_reuse(addr: &str) -> anyhow::Result<tokio::net::TcpListener> {
    let sock_addr: std::net::SocketAddr = addr.parse()?;
    let socket = socket2::Socket::new(
        match sock_addr {
            std::net::SocketAddr::V4(_) => socket2::Domain::IPV4,
            std::net::SocketAddr::V6(_) => socket2::Domain::IPV6,
        },
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&sock_addr.into())?;
    socket.listen(1024)?;
    let std_listener: std::net::TcpListener = socket.into();
    Ok(tokio::net::TcpListener::from_std(std_listener)?)
}
