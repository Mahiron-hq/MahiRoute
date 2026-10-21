// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Platform-independent logic of `MahiRoute`.
//!
//! The crate knows nothing about Android or sockets: it models audio devices, decides which
//! preferences and masks the privileged helper must apply (`plan`), defines the helper wire
//! protocol (`protocol`) and persists user settings (`store`).

pub mod capabilities;
pub mod device;
pub mod plan;
pub mod protocol;
pub mod selection;
pub mod store;

pub use capabilities::{Capabilities, Support};
pub use device::{Device, DeviceKey, DeviceKind, Direction, PortId};
pub use plan::{DesiredRouting, DeviceRef, PlanOutcome, plan_route};
pub use selection::{Choice, Selection};
pub use store::{Settings, StoreError};
