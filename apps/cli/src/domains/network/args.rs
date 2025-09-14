use clap::{Args, Subcommand};
use uuid::Uuid;

use sd_core::{
    domain::addressing::SdPath,
    ops::network::{
        devices::query::ListDevicesQuery,
        pair::{
            cancel::input::PairCancelInput,
            generate::input::PairGenerateInput,
            join::input::PairJoinInput,
        },
        revoke::input::DeviceRevokeInput,
        spacedrop::send::input::SpacedropSendInput,
    },
};

#[derive(Args, Debug, Clone)]
pub struct NetworkDevicesArgs {
    /// Only show paired devices
    #[arg(long, default_value_t = false)]
    pub paired_only: bool,
    /// Only show connected devices
    #[arg(long, default_value_t = false)]
    pub connected_only: bool,
}

impl NetworkDevicesArgs {
    pub fn to_query(&self) -> ListDevicesQuery {
        if self.connected_only {
            ListDevicesQuery::connected()
        } else if self.paired_only {
            ListDevicesQuery::paired()
        } else {
            ListDevicesQuery::all()
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum PairCmd {
    /// Generate a pairing code (initiator)
    Generate {
        #[arg(long, default_value_t = false)]
        auto_accept: bool,
    },
    /// Join using a pairing code (joiner)
    Join {
        code: String,
    },
    /// Show pairing sessions
    Status,
    /// Cancel a pairing session
    Cancel {
        session_id: Uuid,
    },
}

impl PairCmd {
    pub fn to_generate_input(&self) -> Option<PairGenerateInput> {
        match self {
            Self::Generate { auto_accept } => Some(PairGenerateInput { auto_accept: *auto_accept }),
            _ => None,
        }
    }

    pub fn to_join_input(&self) -> Option<PairJoinInput> {
        match self {
            Self::Join { code } => Some(PairJoinInput { code: code.clone() }),
            _ => None,
        }
    }

    pub fn to_cancel_input(&self) -> Option<PairCancelInput> {
        match self {
            Self::Cancel { session_id } => Some(PairCancelInput { session_id: *session_id }),
            _ => None,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct SpacedropArgs {
    /// Target device ID
    pub device_id: Uuid,
    /// Files or directories to share
    pub paths: Vec<String>,
    /// Sender name for display
    #[arg(long)]
    pub sender: Option<String>,
}

impl From<SpacedropArgs> for SpacedropSendInput {
    fn from(args: SpacedropArgs) -> Self {
        let paths = args.paths.iter()
            .map(|s| SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s)))
            .collect();
        Self {
            device_id: args.device_id,
            paths,
            sender: args.sender,
        }
    }
}

#[derive(Args, Debug)]
pub struct RevokeArgs {
    pub device_id: Uuid,
}

impl From<RevokeArgs> for DeviceRevokeInput {
    fn from(args: RevokeArgs) -> Self {
        Self {
            device_id: args.device_id,
        }
    }
}

