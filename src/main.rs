use std::fmt;
use std::net::{IpAddr, Ipv6Addr};
use std::ops::Add;
use std::path::Path;
use std::time::Duration;

use futures::{future, Stream};
use lodepng::{self, Bitmap, RGB};
use tokio_core;
use tokio_ping::Pinger;

const MAX_DIMS: Position = Position { x: 160, y: 120 };
const IP_PREFIX: [u16; 3] = [0x2001, 0x4c08, 0x2028];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dec_to_hex_test() {
        for i in 0..=255 {
            assert_eq!(format!("{}", i), format!("{:x}", dec_to_hex(i)));
        }
    }

    #[test]
    fn pixel_to_ip_addr_test() {
        let pos = Position { x: 160, y: 120 };
        let rgb = RGB {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        };

        assert_eq!(
            format!(
                "{:x}:{:x}:{:x}:{}:{}:{:x}:{:x}:{:x}",
                IP_PREFIX[0], IP_PREFIX[1], IP_PREFIX[2], pos.x, pos.y, rgb.r, rgb.g, rgb.b
            ),
            format!("{}", pixel_to_ip_addr(pos, rgb))
        )
    }

    /*#[test]
    fn ip_vec_test() {
        let expected = vec![
            pixel_to_ip_addr(
                Position { x: 0, y: 0 },
                RGB {
                    r: 0xff,
                    g: 0,
                    b: 0,
                },
            ),
            pixel_to_ip_addr(
                Position { x: 1, y: 0 },
                RGB {
                    r: 0,
                    g: 0xff,
                    b: 0,
                },
            ),
            pixel_to_ip_addr(
                Position { x: 0, y: 1 },
                RGB {
                    r: 0xde,
                    g: 0xad,
                    b: 0xbe,
                },
            ),
            pixel_to_ip_addr(
                Position { x: 1, y: 1 },
                RGB {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                },
            ),
        ];

        assert_eq!(
            Ok(expected),
            ip_vec(Path::new("tests/test.png"), Position { x: 0, y: 0 })
        );

        assert_eq!(
            Err(ConversionError::DimensionsExceeded),
            ip_vec(Path::new("tests/test.png"), Position { x: 160, y: 0 })
        );

        assert_eq!(
            Err(ConversionError::LodePNGError(lodepng::Error(28))),
            ip_vec(Path::new("tests/test.jpg"), Position { x: 160, y: 0 })
        )
    }*/
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Copy)]
struct Position {
    x: usize,
    y: usize,
}

impl Add for Position {
    type Output = Position;

    fn add(self, other: Position) -> Position {
        Position {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

fn dec_to_hex(num: usize) -> u16 {
    let x = num as u16;

    x % 10 + x % 100 / 10 * 0x10 + x / 100 * 0x100

    /*let (mut dec, mut pos, mut hex) = (x, 1, x % 10);

    while dec >= 10 {
        dec /= 10;
        pos *= 16;

        hex += dec % 10 * pos;
    }

    hex*/
}

fn pixel_to_ip_addr(pos: Position, rgb: RGB<u8>) -> IpAddr {
    IpAddr::V6(Ipv6Addr::new(
        IP_PREFIX[0],
        IP_PREFIX[1],
        IP_PREFIX[2],
        dec_to_hex(pos.x),
        dec_to_hex(pos.y),
        u16::from(rgb.r),
        u16::from(rgb.g),
        u16::from(rgb.b),
    ))
}

fn run_pinger(bitmap: Bitmap<RGB<u8>>, pos: Position, timeout: Duration) {
    let mut reactor = tokio_core::reactor::Core::new().unwrap();

    let v: Vec<_> = bitmap
        .buffer
        .chunks_exact(bitmap.width)
        .enumerate()
        .flat_map(|(y, chunk)| {
            chunk
                .iter()
                .enumerate()
                .map(move |(x, rgb)| pixel_to_ip_addr(Position { x, y } + pos, *rgb))
                .map(|addr| {
                    Pinger::new(&reactor.handle())
                        .unwrap()
                        .chain(addr)
                        .timeout(timeout)
                        .stream()
                        .for_each(|_| Ok(()))
                })
        })
        .collect();

    reactor.run(future::join_all(v)).unwrap_or_default();
}

#[derive(Clone, Debug, PartialEq)]
enum ConversionError {
    LodePNGError(lodepng::Error),
    DimensionsExceeded,
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ConversionError::*;

        match &self {
            DimensionsExceeded => write!(f, "image can't fit in given position"),
            LodePNGError(error) => write!(f, "{}", error.as_str()),
        }
    }
}

impl From<lodepng::Error> for ConversionError {
    fn from(err: lodepng::Error) -> Self {
        ConversionError::LodePNGError(err)
    }
}

fn image_to_bitmap(filepath: &Path, pos: Position) -> Result<Bitmap<RGB<u8>>, ConversionError> {
    use self::ConversionError::*;

    let bitmap = lodepng::decode24_file(filepath)?;

    let bottom_right = Position {
        x: bitmap.width,
        y: bitmap.height,
    } + pos;

    if bottom_right >= MAX_DIMS {
        Err(DimensionsExceeded)
    } else {
        Ok(bitmap)
    }
}

fn main() {
    let image_path = Path::new("logo.png");

    let position = Position { x: 109, y: 75 };

    let bitmap = image_to_bitmap(image_path, position).unwrap();

    run_pinger(bitmap, position, Duration::new(0, 100_000));
}
