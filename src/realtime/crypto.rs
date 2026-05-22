//! 체결통보 실시간 데이터 AES-256-CBC 복호화.
//!
//! 알고리즘 출처: docs/kis-api/realtime.md §C-2 (`aes_cbc_base64_dec`).
//! 처리: Base64 디코드 → AES-256-CBC 복호화 → PKCS#7 언패딩 → UTF-8 디코드.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::Engine;

use crate::error::{KisError, Result};

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// 체결통보 구독 응답에서 받은 AES key/iv.
#[derive(Debug, Clone)]
pub(crate) struct AesCreds {
    /// AES-256 secret key — 응답 `body.output.key` 문자열 (UTF-8 32바이트).
    pub key: String,
    /// AES-256 IV — 응답 `body.output.iv` 문자열 (UTF-8 16바이트).
    pub iv: String,
}

impl AesCreds {
    /// Base64 암호문을 복호화해 UTF-8 평문 문자열로 반환.
    pub fn decrypt(&self, cipher_b64: &str) -> Result<String> {
        let key_bytes = self.key.as_bytes();
        let iv_bytes = self.iv.as_bytes();
        if key_bytes.len() != 32 {
            return Err(KisError::Decode(format!(
                "aes key must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }
        if iv_bytes.len() != 16 {
            return Err(KisError::Decode(format!(
                "aes iv must be 16 bytes, got {}",
                iv_bytes.len()
            )));
        }

        let cipher_bytes = base64::engine::general_purpose::STANDARD
            .decode(cipher_b64.trim())
            .map_err(|e| KisError::Decode(format!("base64 decode failed: {e}")))?;

        let dec = Aes256CbcDec::new(key_bytes.into(), iv_bytes.into());
        let plain = dec
            .decrypt_padded_vec_mut::<Pkcs7>(&cipher_bytes)
            .map_err(|e| KisError::Decode(format!("aes-cbc decrypt failed: {e}")))?;

        String::from_utf8(plain)
            .map_err(|e| KisError::Decode(format!("decrypted bytes not utf-8: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut};

    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    /// 라운드트립 자체검증 벡터.
    ///
    /// realtime.md에 KIS 공식 KAT(known-answer-test) 벡터가 없으므로,
    /// 본 테스트는 자체 일관성 검증이다: 알려진 key/iv/평문을 동일 crate로
    /// 암호화 → Base64 → `AesCreds::decrypt`로 복호화 → 원문 일치 확인.
    /// 암호화 결과 Base64 문자열은 한 번 계산해 픽스처로 박아넣어, 복호화
    /// 경로(Base64 디코드·언패딩·UTF-8)가 회귀하면 즉시 깨지도록 한다.
    const KEY: &str = "0123456789abcdef0123456789abcdef"; // 32 bytes
    const IV: &str = "abcdef9876543210"; // 16 bytes
    const PLAIN: &str = "user01^00000000-01^0000123456^^^^^^005930^10^71500^093015^";

    fn encrypt_fixture() -> String {
        let enc = Aes256CbcEnc::new(KEY.as_bytes().into(), IV.as_bytes().into());
        let ct = enc.encrypt_padded_vec_mut::<Pkcs7>(PLAIN.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(ct)
    }

    /// 실행자: `cargo test --lib print_fixture -- --ignored --nocapture`로
    /// 1회 출력 → 아래 EXPECTED_CIPHER_B64에 전사.
    #[test]
    #[ignore = "fixture generator — run once to capture EXPECTED_CIPHER_B64"]
    fn print_fixture() {
        println!("EXPECTED_CIPHER_B64 = {}", encrypt_fixture());
    }

    const EXPECTED_CIPHER_B64: &str =
        "PXkr6elhmjyMym7G5rbAklrBLzv1YHhXxdscX36d4J8egT/jJ7JwONE8hI0nDqwInO9ubUuIxvyOctp5XYxcgw==";

    #[test]
    fn roundtrip_decrypt() {
        let creds = AesCreds {
            key: KEY.into(),
            iv: IV.into(),
        };
        let out = creds.decrypt(EXPECTED_CIPHER_B64).unwrap();
        assert_eq!(out, PLAIN);
        let dynamic = encrypt_fixture();
        assert_eq!(dynamic, EXPECTED_CIPHER_B64, "암호화 결정성 확인");
        assert_eq!(creds.decrypt(&dynamic).unwrap(), PLAIN);
    }

    #[test]
    fn rejects_wrong_key_length() {
        let creds = AesCreds {
            key: "tooshort".into(),
            iv: IV.into(),
        };
        assert!(matches!(
            creds.decrypt(EXPECTED_CIPHER_B64),
            Err(KisError::Decode(_))
        ));
    }
}
