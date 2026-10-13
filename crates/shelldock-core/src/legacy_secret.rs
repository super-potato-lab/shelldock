//! FinalShell legacy DES envelope compatibility. Never use this for ShellDock storage.
//! Protocol behavior and non-sensitive vectors were checked against finalshell2all.
use anyhow::{ensure, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use des::Des;
use md5::{Digest, Md5};
use rand::RngCore;
use zeroize::Zeroizing;

struct JavaRandom(u64);
impl JavaRandom {
    fn new(seed: i64) -> Self {
        Self((seed as u64 ^ 0x5deece66d) & ((1u64 << 48) - 1))
    }
    fn next(&mut self, bits: u32) -> u32 {
        self.0 = self.0.wrapping_mul(0x5deece66d).wrapping_add(11) & ((1u64 << 48) - 1);
        (self.0 >> (48 - bits)) as u32
    }
    fn long(&mut self) -> i64 {
        let high = self.next(32) as i32 as i64;
        let low = self.next(32) as i32 as i64;
        high.wrapping_shl(32).wrapping_add(low)
    }
    fn bounded(&mut self, bound: u32) -> i32 {
        loop {
            let bits = self.next(31);
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) as i32 >= 0 {
                return value as i32;
            }
        }
    }
}

fn derive_key(header: &[u8]) -> Result<[u8; 8]> {
    ensure!(header.len() == 8, "旧版密码头长度无效");
    let b: Vec<i64> = header.iter().map(|x| *x as i8 as i64).collect();
    let divisor = JavaRandom::new(b[5]).bounded(127);
    ensure!(divisor != 0, "旧版密码派生参数无效");
    let mut r = JavaRandom::new(3_680_984_568_597_093_857 / divisor as i64);
    for _ in 0..b[0].max(0) {
        r.long();
    }
    let mut r2 = JavaRandom::new(r.long());
    let values = [b[4], r2.long(), b[7], b[3], r2.long(), b[1], r.long(), b[2]];
    let mut hash = Md5::new();
    for value in values {
        hash.update(value.to_be_bytes());
    }
    Ok(hash.finalize()[..8].try_into()?)
}

pub fn decode(value: &str) -> Result<Zeroizing<String>> {
    if value.trim().is_empty() {
        return Ok(Zeroizing::new(String::new()));
    }
    ensure!(value.len() <= 65536, "密码字段超过兼容大小限制");
    let raw = STANDARD
        .decode(value.trim())
        .map_err(|_| anyhow::anyhow!("旧版密码不是有效 Base64"))?;
    ensure!(
        raw.len() >= 16 && (raw.len() - 8) % 8 == 0,
        "旧版密码长度无效"
    );
    let key = Zeroizing::new(derive_key(&raw[..8])?);
    let cipher = Des::new(GenericArray::from_slice(key.as_ref()));
    let mut plain = Zeroizing::new(raw[8..].to_vec());
    for block in plain.as_chunks_mut::<8>().0 {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    let pad = *plain.last().ok_or_else(|| anyhow::anyhow!("空密码块"))? as usize;
    ensure!(
        pad > 0
            && pad <= 8
            && plain[plain.len() - pad..]
                .iter()
                .all(|x| *x as usize == pad),
        "旧版密码校验失败"
    );
    let unpadded_len = plain.len() - pad;
    plain.truncate(unpadded_len);
    let text =
        std::str::from_utf8(&plain).map_err(|_| anyhow::anyhow!("旧版密码不是有效 UTF-8"))?;
    Ok(Zeroizing::new(text.to_owned()))
}

pub fn encode(value: &str) -> Result<String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    ensure!(value.len() < 65500, "密码过长");
    let mut header = [0u8; 8];
    let key = loop {
        rand::thread_rng().fill_bytes(&mut header);
        if let Ok(key) = derive_key(&header) {
            break Zeroizing::new(key);
        }
    };
    let cipher = Des::new(GenericArray::from_slice(key.as_ref()));
    let pad = 8 - value.len() % 8;
    let mut blocks = Zeroizing::new(value.as_bytes().to_vec());
    blocks.extend(std::iter::repeat_n(pad as u8, pad));
    for block in blocks.as_chunks_mut::<8>().0 {
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }
    let mut raw = header.to_vec();
    raw.extend_from_slice(&blocks);
    Ok(STANDARD.encode(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_independent_reference_vector() {
        assert_eq!(JavaRandom::new(0).long(), -4_962_768_465_676_381_896);
        assert_eq!(
            STANDARD.encode(derive_key(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap()),
            "mJK37zJfK50="
        );
        assert_eq!(
            &*decode("AQIDBAUGBwjPeQvDruVvdza4kcHpV3CH").unwrap(),
            "vector-password"
        );
    }
    #[test]
    fn round_trip_unicode_and_signed_header() {
        for s in ["", "test", "迁移测试🔐", "12345678"] {
            assert_eq!(&*decode(&encode(s).unwrap()).unwrap(), s);
        }
    }
    #[test]
    fn rejects_corrupted_envelopes() {
        for s in [
            "bad%",
            "AQIDBAUGBwge/9bW5zXCzg==",
            "AQIDBAUGBwjbA0r91qPf2w==",
        ] {
            assert!(decode(s).is_err());
        }
        assert!(derive_key(&[0, 0, 0, 0, 0, 52, 0, 0]).is_err());
    }
}
