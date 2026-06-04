use serde::Deserialize;
use serde_json::Value;

use crate::domestic::kis::client::ApiCall;
use crate::domestic::kis::error::Result;
use crate::domestic::kis::futureoption::FutureOption;
use crate::domestic::kis::trid::TrId;

const TR_PRICE: TrId = TrId::same("FHMIF10000000");

/// 선물옵션 현재가 시세 — output1/2/3 통합. futureoption.md §6 통합 응답표 verbatim.
/// chk 코드가 output별 귀속을 분리하지 않으므로 전 필드 `#[serde(default)]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionPrice {
    pub hts_kor_isnm: String,        // HTS 한글 종목명
    pub futs_prpr: String,           // 선물 현재가
    pub futs_prdy_vrss: String,      // 선물 전일 대비
    pub prdy_vrss_sign: String,      // 전일 대비 부호
    pub futs_prdy_clpr: String,      // 선물 전일 종가
    pub futs_prdy_ctrt: String,      // 선물 전일 대비율
    pub acml_vol: String,            // 누적 거래량
    pub acml_tr_pbmn: String,        // 누적 거래 대금
    pub hts_otst_stpl_qty: String,   // HTS 미결제 약정 수량
    pub otst_stpl_qty_icdc: String,  // 미결제 약정 수량 증감
    pub futs_oprc: String,           // 선물 시가2
    pub futs_hgpr: String,           // 선물 최고가
    pub futs_lwpr: String,           // 선물 최저가
    pub futs_mxpr: String,           // 선물 상한가
    pub futs_llam: String,           // 선물 하한가
    pub basis: String,               // 베이시스
    pub futs_sdpr: String,           // 선물 기준가
    pub hts_thpr: String,            // HTS 이론가
    pub dprt: String,                // 괴리율
    pub crbr_aply_mxpr: String,      // 서킷브레이커 적용 상한가
    pub crbr_aply_llam: String,      // 서킷브레이커 적용 하한가
    pub futs_last_tr_date: String,   // 선물 최종 거래 일자
    pub hts_rmnn_dynu: String,       // HTS 잔존 일수
    pub futs_lstn_medm_hgpr: String, // 선물 상장 중 최고가
    pub futs_lstn_medm_lwpr: String, // 선물 상장 중 최저가
    pub delta_val: String,           // 델타 값
    pub gama: String,                // 감마
    pub theta: String,               // 세타
    pub vega: String,                // 베가
    pub rho: String,                 // 로우
    pub hist_vltl: String,           // 역사적 변동성
    pub hts_ints_vltl: String,       // HTS 내재 변동성
    pub mrkt_basis: String,          // 시장 베이시스
    pub acpr: String,                // 행사가
    pub bstp_cls_code: String,       // 업종 구분 코드
    pub bstp_nmix_prpr: String,      // 업종 지수 현재가
    pub bstp_nmix_prdy_vrss: String, // 업종 지수 전일 대비
    pub bstp_nmix_prdy_ctrt: String, // 업종 지수 전일 대비율
}

const TR_ASKING: TrId = TrId::same("FHMIF10010000");

/// 선물옵션 호가 — output1/2 통합. futureoption.md §7 통합 응답표 verbatim.
/// 호가 5단계 배열 필드는 1~5 개별 필드로 전사. 전 필드 `#[serde(default)]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionAskingPrice {
    pub hts_kor_isnm: String,    // HTS 한글 종목명
    pub futs_prpr: String,       // 선물 현재가
    pub prdy_vrss_sign: String,  // 전일 대비 부호
    pub futs_prdy_vrss: String,  // 선물 전일 대비
    pub futs_prdy_ctrt: String,  // 선물 전일 대비율
    pub acml_vol: String,        // 누적 거래량
    pub futs_prdy_clpr: String,  // 선물 전일 종가
    pub futs_shrn_iscd: String,  // 선물 단축 종목코드
    pub futs_askp1: String,      // 선물 매도호가1
    pub futs_askp2: String,      // 선물 매도호가2
    pub futs_askp3: String,      // 선물 매도호가3
    pub futs_askp4: String,      // 선물 매도호가4
    pub futs_askp5: String,      // 선물 매도호가5
    pub futs_bidp1: String,      // 선물 매수호가1
    pub futs_bidp2: String,      // 선물 매수호가2
    pub futs_bidp3: String,      // 선물 매수호가3
    pub futs_bidp4: String,      // 선물 매수호가4
    pub futs_bidp5: String,      // 선물 매수호가5
    pub askp_rsqn1: String,      // 매도호가 잔량1
    pub askp_rsqn2: String,      // 매도호가 잔량2
    pub askp_rsqn3: String,      // 매도호가 잔량3
    pub askp_rsqn4: String,      // 매도호가 잔량4
    pub askp_rsqn5: String,      // 매도호가 잔량5
    pub bidp_rsqn1: String,      // 매수호가 잔량1
    pub bidp_rsqn2: String,      // 매수호가 잔량2
    pub bidp_rsqn3: String,      // 매수호가 잔량3
    pub bidp_rsqn4: String,      // 매수호가 잔량4
    pub bidp_rsqn5: String,      // 매수호가 잔량5
    pub askp_csnu1: String,      // 매도호가 건수1
    pub askp_csnu2: String,      // 매도호가 건수2
    pub askp_csnu3: String,      // 매도호가 건수3
    pub askp_csnu4: String,      // 매도호가 건수4
    pub askp_csnu5: String,      // 매도호가 건수5
    pub bidp_csnu1: String,      // 매수호가 건수1
    pub bidp_csnu2: String,      // 매수호가 건수2
    pub bidp_csnu3: String,      // 매수호가 건수3
    pub bidp_csnu4: String,      // 매수호가 건수4
    pub bidp_csnu5: String,      // 매수호가 건수5
    pub total_askp_rsqn: String, // 총 매도호가 잔량
    pub total_bidp_rsqn: String, // 총 매수호가 잔량
    pub total_askp_csnu: String, // 총 매도호가 건수
    pub total_bidp_csnu: String, // 총 매수호가 건수
    pub aspr_acpt_hour: String,  // 호가 접수 시간
}

/// FID 시장 분류 코드. F=지수선물 / O=지수옵션 (현재가) / JF=주식선물 (호가).
#[derive(Debug, Clone, Copy)]
pub enum MarketDiv {
    /// 지수선물 (`F`).
    IndexFuture,
    /// 지수옵션 (`O`).
    IndexOption,
    /// 주식선물 (`JF`).
    StockFuture,
}

impl MarketDiv {
    fn code(self) -> &'static str {
        match self {
            MarketDiv::IndexFuture => "F",
            MarketDiv::IndexOption => "O",
            MarketDiv::StockFuture => "JF",
        }
    }
}

/// body의 output1/2/3를 하나의 통합 struct로 병합 역직렬화하는 헬퍼.
fn merge_outputs<T: serde::de::DeserializeOwned>(body: &Value) -> Result<T> {
    let mut merged = serde_json::Map::new();
    for key in ["output1", "output2", "output3"] {
        if let Some(Value::Object(o)) = body.get(key) {
            for (k, v) in o {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(serde_json::from_value(Value::Object(merged))?)
}

impl FutureOption<'_> {
    /// 선물옵션 현재가 시세 (TR 6). output1/2/3 병합 통합 struct + raw body 반환.
    pub async fn current_price(
        &self,
        market: MarketDiv,
        item_code: &str,
    ) -> Result<(FutureOptionPrice, Value)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/quotations/inquire-price".into(),
                tr_id: TR_PRICE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.code(),
                    "FID_INPUT_ISCD": item_code,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let merged = merge_outputs(&resp.body)?;
        Ok((merged, resp.body))
    }

    /// 선물옵션 시세호가 (TR 7). output1/2 병합 통합 struct + raw body 반환.
    pub async fn asking_price(
        &self,
        market: MarketDiv,
        item_code: &str,
    ) -> Result<(FutureOptionAskingPrice, Value)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/quotations/inquire-asking-price".into(),
                tr_id: TR_ASKING.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.code(),
                    "FID_INPUT_ISCD": item_code,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let merged = merge_outputs(&resp.body)?;
        Ok((merged, resp.body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_outputs_combines_keys() {
        let body = serde_json::json!({
            "rt_cd": "0",
            "output1": { "futs_prpr": "350.05", "acml_vol": "12000" },
            "output3": { "bstp_nmix_prpr": "2500.10" }
        });
        let merged: FutureOptionPrice = merge_outputs(&body).unwrap();
        assert_eq!(merged.futs_prpr, "350.05");
        assert_eq!(merged.acml_vol, "12000");
        assert_eq!(merged.bstp_nmix_prpr, "2500.10");
        assert_eq!(merged.delta_val, "");
    }
}
