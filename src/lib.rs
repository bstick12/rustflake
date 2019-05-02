#![feature(atomic_min_max)]
#![feature(integer_atomics)]
#![feature(test)]

extern crate base64;
extern crate interfaces;
extern crate test;

use std::cmp;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct Generator {
    seed: [u8; 6],
    sequence: AtomicU64,
    timestamp: AtomicU64,
}

impl PartialEq for Generator {
    fn eq(&self, other: &Generator) -> bool {
        self.seed == other.seed
            && self.sequence.load(Ordering::SeqCst) == other.sequence.load(Ordering::SeqCst)
    }
}

pub trait SnowFlaker {
    fn new() -> Self;
    fn with_seed(seed: [u8; 6]) -> Self;
    fn generate(&self) -> String;
}

impl SnowFlaker for Generator {
    fn new() -> Generator {
        Generator::with_seed(get_non_loopback_address())
    }

    fn with_seed(seed: [u8; 6]) -> Generator {
        Generator {
            seed: seed,
            sequence: AtomicU64::new(0),
            timestamp: AtomicU64::new(0),
        }
    }

    fn generate(&self) -> String {
        let now = SystemTime::now();
        let since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
        let since_epoch_in_ms = since_epoch.as_millis() as u64;
        let previous_value = self
            .timestamp
            .fetch_max(since_epoch_in_ms, Ordering::Relaxed);
        let max = cmp::max(previous_value, since_epoch_in_ms);
        let mut flake_id = [0; 15];
        put_uint(&mut flake_id, max, 0, 6);

        copy_seed(&mut flake_id, self.seed);

        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);
        put_uint(&mut flake_id, sequence, 12, 3);

        base64::encode_config(&flake_id, base64::URL_SAFE)
    }
}

fn put_uint(byte_array: &mut [u8], long_value: u64, pos: u8, number_of_bytes: u8) {
    for i in 0..number_of_bytes {
        let val = (long_value >> i * 8) as u8;
        let index = (pos + number_of_bytes - i - 1) as usize;
        byte_array[index] = val;
    }
}

fn copy_seed(byte_array: &mut [u8], seed_array: [u8; 6]) {
    for i in 0..seed_array.len() {
        byte_array[i + 6] = seed_array[i];
    }
}

pub fn get_non_loopback_address() -> [u8; 6] {
    let interfaces = interfaces::Interface::get_all();
    match interfaces {
        Ok(vector) => {
            for interface in vector {
                if !interface.is_loopback() && interface.is_up() {
                    let hardware_addr = interface.hardware_addr().unwrap();
                    let mut bytes = [0; 6];
                    bytes[..6].clone_from_slice(&hardware_addr.as_bytes());
                    return bytes;
                }
            }
            panic!("Can't find an suitable interface address")
        }
        Err(_e) => panic!("Error retrieving interfaces"),
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::collections::HashSet;
    use test::Bencher;

    #[test]
    fn test_with_seed() {
        assert_eq!(
            Generator::with_seed([0; 6]),
            Generator {
                seed: [0; 6],
                sequence: AtomicU64::new(0),
                timestamp: AtomicU64::new(0)
            }
        );
    }

    #[test]
    fn test_generate_value() {
        let generator = Generator::new();
        let decoded = base64::decode_config(&generator.generate(), base64::URL_SAFE);
        assert!(decoded.is_ok())
    }

    #[test]
    fn test_subsequent_generate_lexically_greater_values() {
        let generator = Generator::new();
        let first_value = generator.generate();
        let second_value = generator.generate();
        assert!(
            first_value < second_value,
            "Expect subsequently generated values to be lexically greater than each other {} {}",
            first_value,
            second_value
        );
        println!("first value = {}", first_value);
        println!("second value = {}", second_value);
    }

    #[test]
    fn test_subsequent_generate_calls_produce_different_values() {
        let mut set = HashSet::new();
        let generator = Generator::new();

        for _x in 0..100000 {
            let generated = generator.generate();
            assert!(set.insert(generated));
        }
    }

    #[bench]
    fn bench_generator(b: &mut Bencher) {
        let generator = Generator::new();
        b.iter(|| generator.generate());
    }

    #[bench]
    fn bench_generator_100000(b: &mut Bencher) {
        let generator = test::black_box(Generator::new());
        b.iter(|| {
            for _x in 0..100000 {
                generator.generate();
            }
        });
    }

}
