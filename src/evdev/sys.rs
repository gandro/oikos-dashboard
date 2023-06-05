use nix::ioctl_read_buf;

include!(concat!(env!("OUT_DIR"), "/input-event-codes.rs"));

pub const EV_KEY: u16 = 1;
pub const EV_CNT: u16 = 32;

const EVIO_IOC_MAGIC: u8 = b'E';
const EVIOCGNAME: u8 = 0x06;
const EVIOCGBIT: u8 = 0x20;

ioctl_read_buf!(evdev_get_name, EVIO_IOC_MAGIC, EVIOCGNAME, u8);
ioctl_read_buf!(evdev_get_event_bits, EVIO_IOC_MAGIC, EVIOCGBIT, u8);
ioctl_read_buf!(evdev_get_event_key_bits, EVIO_IOC_MAGIC, EVIOCGBIT + EV_KEY as u8, u8);
