pub mod mouse;
pub mod window;
pub mod keyboard;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct KeyEventExtra;


impl DeviceId {
    pub const fn dummy() -> Self {
        DeviceId
    }
}

