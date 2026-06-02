//! 국내주식 순위분석 TR — 거래량/거래대금 순위 (시장 스캐너).
//!
//! 종목별 조회(`quote`/`flow`)와 달리 **시장 전체를 한 번에 랭킹**한다.
//! 순환매 레이더의 눈 — "지금 어디로 거래대금이 몰리는가"를 스캔.
//!
//! 응답 필드는 `koreainvestment/open-trading-api`
//! `examples_llm/domestic_stock/volume_rank` 샘플 COLUMN_MAPPING에서 추출(추측 없음).
//! 알파 핵심 필드만 타입화, `#[serde(default)]`로 누락 내성.

use serde::Deserialize;

use crate::kis::client::ApiCall;
use crate::kis::domestic_stock::{DomesticStock, Market};
use crate::kis::error::{KisError, Result};
use crate::kis::trid::TrId;

const TR_VOLUME_RANK: TrId = TrId::same("FHPST01710000");

/// 거래량순위 정렬 기준 — `FID_BLNG_CLS_CODE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankBy {
    /// 0 — 평균거래량.
    AvgVolume,
    /// 1 — 거래량 증가율.
    VolumeIncreaseRate,
    /// 2 — 평균거래 회전율.
    AvgTurnoverRate,
    /// 3 — 거래대금순. 순환매 레이더의 기본 — 돈이 몰리는 순서.
    TradingAmount,
    /// 4 — 평균거래대금 회전율.
    AvgAmountTurnoverRate,
}

impl RankBy {
    fn code(self) -> &'static str {
        match self {
            RankBy::AvgVolume => "0",
            RankBy::VolumeIncreaseRate => "1",
            RankBy::AvgTurnoverRate => "2",
            RankBy::TradingAmount => "3",
            RankBy::AvgAmountTurnoverRate => "4",
        }
    }
}

/// 거래량순위 1행 — `output` 배열 요소.
///
/// `acml_tr_pbmn`(누적 거래대금)이 순환매 신호의 연료. `data_rank`는 정렬 순위.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct VolumeRankItem {
    /// HTS 한글 종목명.
    pub hts_kor_isnm: String,
    /// 유가증권 단축 종목코드.
    pub mksc_shrn_iscd: String,
    /// 데이터 순위.
    pub data_rank: String,
    /// 주식 현재가.
    pub stck_prpr: String,
    /// 전일 대비 부호.
    pub prdy_vrss_sign: String,
    /// 전일 대비.
    pub prdy_vrss: String,
    /// 전일 대비율.
    pub prdy_ctrt: String,
    /// 누적 거래량.
    pub acml_vol: String,
    /// 전일 거래량.
    pub prdy_vol: String,
    /// 상장 주수.
    pub lstn_stcn: String,
    /// 평균 거래량.
    pub avrg_vol: String,
    /// N일 전 종가 대비 현재가 비율.
    pub n_befr_clpr_vrss_prpr_rate: String,
    /// 거래량 증가율.
    pub vol_inrt: String,
    /// 거래량 회전율.
    pub vol_tnrt: String,
    /// N일 거래량 회전율.
    pub nday_vol_tnrt: String,
    /// 평균 거래대금.
    pub avrg_tr_pbmn: String,
    /// 거래대금 회전율.
    pub tr_pbmn_tnrt: String,
    /// N일 거래대금 회전율.
    pub nday_tr_pbmn_tnrt: String,
    /// 누적 거래대금 — 순환매 연료.
    pub acml_tr_pbmn: String,
}

impl DomesticStock<'_> {
    /// 거래량/거래대금 순위 — TR `FHPST01710000`.
    ///
    /// 시장 전체를 `rank_by` 기준으로 랭킹. `market`은 KRX(`J`) 또는 NXT(`NX`)만 지원 —
    /// **통합(`Unified`)은 KIS가 거부**(`OPSQ2001 INVALID FID_COND_MRKT_DIV_CODE`, 2026-06-02
    /// 실API 확인). 통합 거래대금 스캔은 KRX/NXT 각각 조회해 호출자가 병합.
    /// 최대 30위 반환(KIS 사양). 전체 종목 대상(`FID_INPUT_ISCD="0000"`).
    pub async fn volume_rank(
        &self,
        market: Market,
        rank_by: RankBy,
    ) -> Result<Vec<VolumeRankItem>> {
        if market == Market::Unified {
            return Err(KisError::Decode(
                "volume_rank는 통합(Unified) 미지원 — KRX 또는 NXT만 (KIS OPSQ2001)".into(),
            ));
        }
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/volume-rank".into(),
                tr_id: TR_VOLUME_RANK.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_COND_SCR_DIV_CODE": "20171",
                    "FID_INPUT_ISCD": "0000",
                    "FID_DIV_CLS_CODE": "0",
                    "FID_BLNG_CLS_CODE": rank_by.code(),
                    "FID_TRGT_CLS_CODE": "0",
                    "FID_TRGT_EXLS_CLS_CODE": "0",
                    "FID_INPUT_PRICE_1": "",
                    "FID_INPUT_PRICE_2": "",
                    "FID_VOL_CNT": "",
                    "FID_INPUT_DATE_1": "",
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_by_codes() {
        assert_eq!(RankBy::AvgVolume.code(), "0");
        assert_eq!(RankBy::VolumeIncreaseRate.code(), "1");
        assert_eq!(RankBy::AvgTurnoverRate.code(), "2");
        assert_eq!(RankBy::TradingAmount.code(), "3");
        assert_eq!(RankBy::AvgAmountTurnoverRate.code(), "4");
    }

    #[test]
    fn volume_rank_item_serde_default() {
        // KIS는 명세 필드도 실응답에서 누락 가능 — 부분 JSON 내성 확인.
        let partial = serde_json::json!({
            "mksc_shrn_iscd": "005930",
            "hts_kor_isnm": "삼성전자",
            "data_rank": "1",
            "acml_tr_pbmn": "1234567890",
        });
        let item: VolumeRankItem = serde_json::from_value(partial).unwrap();
        assert_eq!(item.mksc_shrn_iscd, "005930");
        assert_eq!(item.hts_kor_isnm, "삼성전자");
        assert_eq!(item.data_rank, "1");
        assert_eq!(item.acml_tr_pbmn, "1234567890");
        // 누락 필드는 빈 문자열.
        assert_eq!(item.stck_prpr, "");
        assert_eq!(item.vol_inrt, "");
    }

    #[test]
    fn volume_rank_item_full_serde() {
        let full = serde_json::json!({
            "hts_kor_isnm": "에코프로비엠",
            "mksc_shrn_iscd": "247540",
            "data_rank": "2",
            "stck_prpr": "180000",
            "prdy_vrss_sign": "2",
            "prdy_vrss": "5000",
            "prdy_ctrt": "2.86",
            "acml_vol": "3500000",
            "prdy_vol": "2100000",
            "lstn_stcn": "97801344",
            "avrg_vol": "2800000",
            "n_befr_clpr_vrss_prpr_rate": "1.20",
            "vol_inrt": "66.67",
            "vol_tnrt": "3.58",
            "nday_vol_tnrt": "12.10",
            "avrg_tr_pbmn": "480000000000",
            "tr_pbmn_tnrt": "5.20",
            "nday_tr_pbmn_tnrt": "18.30",
            "acml_tr_pbmn": "630000000000"
        });
        let item: VolumeRankItem = serde_json::from_value(full).unwrap();
        assert_eq!(item.data_rank, "2");
        assert_eq!(item.acml_tr_pbmn, "630000000000");
        assert_eq!(item.vol_inrt, "66.67");
    }
}
