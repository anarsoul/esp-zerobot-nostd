pub const MAGIC: u32 = 0xDEAD_BEEF;
pub const REVISION: u32 = 1;
pub const PACKET_SIZE: usize = 20;

#[derive(Debug, Clone, Copy)]
pub struct TelemetryPacket {
    pub battery_mv: u16,
    pub left_duty: u8,
    pub right_duty: u8,
    pub left_pulses: u32,
    pub right_pulses: u32,
}

#[allow(dead_code)]
pub fn pack(pkt: &TelemetryPacket) -> [u8; PACKET_SIZE] {
    let mut buf = [0u8; PACKET_SIZE];
    buf[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    buf[4..8].copy_from_slice(&REVISION.to_le_bytes());
    buf[8..10].copy_from_slice(&pkt.battery_mv.to_le_bytes());
    buf[10] = pkt.left_duty;
    buf[11] = pkt.right_duty;
    buf[12..16].copy_from_slice(&pkt.left_pulses.to_le_bytes());
    buf[16..20].copy_from_slice(&pkt.right_pulses.to_le_bytes());
    buf
}

#[allow(dead_code)]
pub fn unpack(buf: &[u8]) -> Option<TelemetryPacket> {
    if buf.len() < PACKET_SIZE {
        return None;
    }
    let magic = u32::from_le_bytes(buf[0..4].try_into().ok()?);
    if magic != MAGIC {
        return None;
    }
    let revision = u32::from_le_bytes(buf[4..8].try_into().ok()?);
    if revision != REVISION {
        return None;
    }
    Some(TelemetryPacket {
        battery_mv: u16::from_le_bytes(buf[8..10].try_into().ok()?),
        left_duty: buf[10],
        right_duty: buf[11],
        left_pulses: u32::from_le_bytes(buf[12..16].try_into().ok()?),
        right_pulses: u32::from_le_bytes(buf[16..20].try_into().ok()?),
    })
}
