//! 한국 주식 종목 어휘(vocabulary). venue 비종속 식별자만 정의한다.
//!
//! 각 거래소의 심볼 문자열(예 Binance `SAMSUNGUSDT`, Hyperliquid `xyz:SMSN`)은
//! **해당 venue 모듈**이 [`KrStock`]→심볼로 매핑한다(`global::binance::symbol` 등).
//! 모든 venue가 모든 종목을 상장하진 않으므로 매핑은 `Option`을 돌려준다. 이 파일은
//! 종목 정체성(KRX 코드·이름)만 들고 venue를 모른다 — 신규 venue 추가가 이 파일을
//! 건드리지 않게 하기 위함(독립 모듈 원칙 유지).

/// perp이 상장된 한국 주식 종목.
///
/// 변형 추가 시 KRX 코드·한글/영문명을 채우고, 상장한 venue 모듈의 `symbol` 매핑에
/// 한 줄을 더한다. 미상장 venue는 손대지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum KrStock {
    /// 삼성전자 (KRX 005930).
    SamsungElec,
    /// SK하이닉스 (KRX 000660).
    SkHynix,
    /// 현대차 (KRX 005380).
    HyundaiMotor,
    /// KOSPI 200 지수. 단일종목이 아닌 지수 perp([`KrStock::is_index`]).
    /// venue별 표기 상이: HL `xyz:KR200`, Lighter `KRCOMP`(Korea-composite),
    /// BingX `NCSIKOSPI2USD-USDT`.
    Kospi200,
}

impl KrStock {
    /// 현재 정의된 모든 종목·지수.
    pub const ALL: &'static [KrStock] = &[
        KrStock::SamsungElec,
        KrStock::SkHynix,
        KrStock::HyundaiMotor,
        KrStock::Kospi200,
    ];

    /// 지수 종목 여부(단일주식=false). KRX 종목코드가 없는 합성/지수 상품.
    pub const fn is_index(self) -> bool {
        matches!(self, KrStock::Kospi200)
    }

    /// KRX 6자리 종목코드. 지수는 코드가 없어 `None`.
    pub const fn krx_code(self) -> Option<&'static str> {
        Some(match self {
            KrStock::SamsungElec => "005930",
            KrStock::SkHynix => "000660",
            KrStock::HyundaiMotor => "005380",
            KrStock::Kospi200 => return None,
        })
    }

    /// 한글 종목명.
    pub const fn korean_name(self) -> &'static str {
        match self {
            KrStock::SamsungElec => "삼성전자",
            KrStock::SkHynix => "SK하이닉스",
            KrStock::HyundaiMotor => "현대차",
            KrStock::Kospi200 => "코스피200",
        }
    }

    /// 영문 종목명.
    pub const fn english_name(self) -> &'static str {
        match self {
            KrStock::SamsungElec => "Samsung Electronics",
            KrStock::SkHynix => "SK Hynix",
            KrStock::HyundaiMotor => "Hyundai Motor",
            KrStock::Kospi200 => "KOSPI 200",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_distinct_codes() {
        let codes: Vec<_> = KrStock::ALL.iter().filter_map(|s| s.krx_code()).collect();
        let mut uniq = codes.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(codes.len(), uniq.len(), "KRX 코드 중복");
    }

    #[test]
    fn names_present() {
        for s in KrStock::ALL {
            assert!(!s.korean_name().is_empty());
            assert!(!s.english_name().is_empty());
            // 단일종목은 6자리 KRX 코드, 지수는 코드 없음.
            match s.krx_code() {
                Some(code) => assert_eq!(code.len(), 6),
                None => assert!(s.is_index()),
            }
        }
    }
}
