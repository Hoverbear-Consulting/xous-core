pub(crate) const SERVER_NAME_USB_DEVICE: &'static str = "_Xous USB device driver_";

#[derive(num_derive::FromPrimitive, num_derive::ToPrimitive, Debug)]
pub(crate) enum Opcode {
    /// Returns the link status
    LinkStatus,
    /// Send a keyboard code
    SendKeyCode,
    /// "Type" a string to the keyboard
    SendString,
    /// Get the current LED state
    GetLedState,
    /// Switch to a specified device core
    SwitchCores,
    /// Check which core is connected
    WhichCore,
    /// Restrict the debug core
    RestrictDebugAccess,
    /// Retrieve restriction state
    IsRestricted,
    /// Set-and-check of USB debug restriction
    DebugUsbOp,

    /// Send a U2F message
    U2fTx,
    /// Blocks the caller, waiting for a U2F message
    U2fRxDeferred,

    /// Handle the USB interrupt
    UsbIrqHandler,
    /// Suspend/resume callback
    SuspendResume,
    /// Exits the server
    Quit,
}

#[derive(Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Copy, Clone)]
pub struct UsbString {
    pub s: xous_ipc::String::<4000>,
    pub sent: Option<u32>,
}

#[derive(Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Copy, Clone)]
pub struct U2fMsgIpc {
    /// All U2F protocol messages are 64 bytes
    pub data: [u8; 64],
    /// Encodes the state of the message
    pub code: U2fCode,
}

#[derive(Debug, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Copy, Clone, Eq, PartialEq)]
pub enum U2fCode {
    Tx,
    TxAck,
    RxWait,
    RxAck,
    Denied,
}