// Copyright 2021-2022 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("{received} is not a valid packet mode tag")]
pub struct InvalidPacketType {
    received: u8,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PacketType {
    /// Represents 'normal' packet sent through the network that should be delayed by an appropriate
    /// value at each hop.
    #[default]
    Mix = 0,

    /// Represents a packet that should be sent through the network as fast as possible.
    Vpn = 1,

    /// Abusing this to add Outfox support
    Outfox = 2,
}

impl fmt::Display for PacketType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PacketType::Mix => write!(f, "Mix"),
            PacketType::Vpn => write!(f, "Vpn"),
            PacketType::Outfox => write!(f, "Outfox"),
        }
    }
}

impl PacketType {
    pub fn is_mix(self) -> bool {
        self == PacketType::Mix
    }

    pub fn is_outfox(self) -> bool {
        self == PacketType::Outfox
    }
}

impl TryFrom<u8> for PacketType {
    type Error = InvalidPacketType;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            _ if value == (PacketType::Mix as u8) => Ok(Self::Mix),
            _ if value == (PacketType::Outfox as u8) => Ok(Self::Outfox),
            v => Err(InvalidPacketType { received: v }),
        }
    }
}
