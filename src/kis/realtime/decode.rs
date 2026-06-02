//! 수신 프레임 파싱 + tr_id별 필드 매핑.
//!
//! 프레임 구분자 2단계: 프레임 `|`, 필드 `^` (docs/kis-api/realtime.md §A-5).
//! 한 프레임에 data_cnt건이 올 수 있어 본문을 `필드수 × 건수`로 청크 분할한다.

use crate::kis::error::{KisError, Result};
use crate::kis::realtime::crypto::AesCreds;

/// 국내주식 실시간체결가 (H0STCNT0). 46개 필드.
/// 필드표 전체는 docs/kis-api/realtime.md §B-1 — 순번 1~46 verbatim.
#[derive(Debug, Clone)]
pub struct StockTrade {
    pub mksc_shrn_iscd: String,               // 1 유가증권 단축 종목코드
    pub stck_cntg_hour: String,               // 2 주식 체결 시간
    pub stck_prpr: String,                    // 3 주식 현재가
    pub prdy_vrss_sign: String,               // 4 전일 대비 부호
    pub prdy_vrss: String,                    // 5 전일 대비
    pub prdy_ctrt: String,                    // 6 전일 대비율
    pub wghn_avrg_stck_prc: String,           // 7 가중 평균 주식 가격
    pub stck_oprc: String,                    // 8 주식 시가
    pub stck_hgpr: String,                    // 9 주식 최고가
    pub stck_lwpr: String,                    // 10 주식 최저가
    pub askp1: String,                        // 11 매도호가1
    pub bidp1: String,                        // 12 매수호가1
    pub cntg_vol: String,                     // 13 체결 거래량
    pub acml_vol: String,                     // 14 누적 거래량
    pub acml_tr_pbmn: String,                 // 15 누적 거래대금
    pub seln_cntg_csnu: String,               // 16 매도 체결 건수
    pub shnu_cntg_csnu: String,               // 17 매수 체결 건수
    pub ntby_cntg_csnu: String,               // 18 순매수 체결 건수
    pub cttr: String,                         // 19 체결강도
    pub seln_cntg_smtn: String,               // 20 총 매도 수량
    pub shnu_cntg_smtn: String,               // 21 총 매수 수량
    pub ccld_dvsn: String,                    // 22 체결 구분
    pub shnu_rate: String,                    // 23 매수비율
    pub prdy_vol_vrss_acml_vol_rate: String,  // 24 전일 거래량 대비 등락율
    pub oprc_hour: String,                    // 25 시가 시간
    pub oprc_vrss_prpr_sign: String,          // 26 시가 대비 구분
    pub oprc_vrss_prpr: String,               // 27 시가 대비
    pub hgpr_hour: String,                    // 28 최고가 시간
    pub hgpr_vrss_prpr_sign: String,          // 29 고가 대비 구분
    pub hgpr_vrss_prpr: String,               // 30 고가 대비
    pub lwpr_hour: String,                    // 31 최저가 시간
    pub lwpr_vrss_prpr_sign: String,          // 32 저가 대비 구분
    pub lwpr_vrss_prpr: String,               // 33 저가 대비
    pub bsop_date: String,                    // 34 영업 일자
    pub new_mkop_cls_code: String,            // 35 신 장운영 구분 코드
    pub trht_yn: String,                      // 36 거래정지 여부
    pub askp_rsqn1: String,                   // 37 매도호가 잔량1
    pub bidp_rsqn1: String,                   // 38 매수호가 잔량1
    pub total_askp_rsqn: String,              // 39 총 매도호가 잔량
    pub total_bidp_rsqn: String,              // 40 총 매수호가 잔량
    pub vol_tnrt: String,                     // 41 거래량 회전율
    pub prdy_smns_hour_acml_vol: String,      // 42 전일 동시간 누적 거래량
    pub prdy_smns_hour_acml_vol_rate: String, // 43 전일 동시간 누적 거래량 비율
    pub hour_cls_code: String,                // 44 시간 구분 코드
    pub mrkt_trtm_cls_code: String,           // 45 임의종료 구분 코드
    pub vi_stnd_prc: String,                  // 46 정적 VI 발동기준가
}

/// 국내주식 실시간호가 (H0STASP0). 59개 필드 (idx 0~58).
/// 필드표 전체는 docs/kis-api/realtime.md §B-2 — 순번 1~59 verbatim.
#[derive(Debug, Clone)]
pub struct StockAsking {
    pub mksc_shrn_iscd: String,            // idx 0 유가증권 단축 종목코드
    pub bsop_hour: String,                 // idx 1 영업시간
    pub hour_cls_code: String,             // idx 2 시간구분코드
    pub askp1: String,                     // idx 3 매도호가01
    pub askp2: String,                     // idx 4 매도호가02
    pub askp3: String,                     // idx 5 매도호가03
    pub askp4: String,                     // idx 6 매도호가04
    pub askp5: String,                     // idx 7 매도호가05
    pub askp6: String,                     // idx 8 매도호가06
    pub askp7: String,                     // idx 9 매도호가07
    pub askp8: String,                     // idx 10 매도호가08
    pub askp9: String,                     // idx 11 매도호가09
    pub askp10: String,                    // idx 12 매도호가10
    pub bidp1: String,                     // idx 13 매수호가01
    pub bidp2: String,                     // idx 14 매수호가02
    pub bidp3: String,                     // idx 15 매수호가03
    pub bidp4: String,                     // idx 16 매수호가04
    pub bidp5: String,                     // idx 17 매수호가05
    pub bidp6: String,                     // idx 18 매수호가06
    pub bidp7: String,                     // idx 19 매수호가07
    pub bidp8: String,                     // idx 20 매수호가08
    pub bidp9: String,                     // idx 21 매수호가09
    pub bidp10: String,                    // idx 22 매수호가10
    pub askp_rsqn1: String,                // idx 23 매도호가 잔량01
    pub askp_rsqn2: String,                // idx 24 매도호가 잔량02
    pub askp_rsqn3: String,                // idx 25 매도호가 잔량03
    pub askp_rsqn4: String,                // idx 26 매도호가 잔량04
    pub askp_rsqn5: String,                // idx 27 매도호가 잔량05
    pub askp_rsqn6: String,                // idx 28 매도호가 잔량06
    pub askp_rsqn7: String,                // idx 29 매도호가 잔량07
    pub askp_rsqn8: String,                // idx 30 매도호가 잔량08
    pub askp_rsqn9: String,                // idx 31 매도호가 잔량09
    pub askp_rsqn10: String,               // idx 32 매도호가 잔량10
    pub bidp_rsqn1: String,                // idx 33 매수호가 잔량01
    pub bidp_rsqn2: String,                // idx 34 매수호가 잔량02
    pub bidp_rsqn3: String,                // idx 35 매수호가 잔량03
    pub bidp_rsqn4: String,                // idx 36 매수호가 잔량04
    pub bidp_rsqn5: String,                // idx 37 매수호가 잔량05
    pub bidp_rsqn6: String,                // idx 38 매수호가 잔량06
    pub bidp_rsqn7: String,                // idx 39 매수호가 잔량07
    pub bidp_rsqn8: String,                // idx 40 매수호가 잔량08
    pub bidp_rsqn9: String,                // idx 41 매수호가 잔량09
    pub bidp_rsqn10: String,               // idx 42 매수호가 잔량10
    pub total_askp_rsqn: String,           // idx 43 총 매도호가 잔량
    pub total_bidp_rsqn: String,           // idx 44 총 매수호가 잔량
    pub ovtm_total_askp_rsqn: String,      // idx 45 시간외 총 매도호가 잔량
    pub ovtm_total_bidp_rsqn: String,      // idx 46 시간외 총 매수호가 잔량
    pub antc_cnpr: String,                 // idx 47 예상 체결가
    pub antc_cntg_vol: String,             // idx 48 예상 체결량
    pub antc_vol: String,                  // idx 49 예상 거래량
    pub antc_cntg_vrss: String,            // idx 50 예상체결 대비
    pub antc_cntg_vrss_sign: String,       // idx 51 부호
    pub antc_cntg_prdy_ctrt: String,       // idx 52 예상체결 전일대비율
    pub acml_vol: String,                  // idx 53 누적 거래량
    pub total_askp_rsqn_icdc: String,      // idx 54 총 매도호가 잔량 증감
    pub total_bidp_rsqn_icdc: String,      // idx 55 총 매수호가 잔량 증감
    pub ovtm_total_askp_rsqn_icdc: String, // idx 56 시간외 총 매도호가 잔량 (증감)
    pub ovtm_total_bidp_rsqn_icdc: String, // idx 57 시간외 총 매수호가 잔량 (증감)
    pub stck_bsop_cls_code: String,        // idx 58 주식매매 구분코드
    // idx 59~64: NXT/통합(H0NXASP0/H0UNASP0)만 존재 — 중간가호가. KRX(H0STASP0)는 빈 String.
    pub kmid_prc: String,        // idx 59 KRX 중간가
    pub kmid_total_rsqn: String, // idx 60 KRX 중간가 총잔량
    pub kmid_cls_code: String,   // idx 61 KRX 중간가 구분코드
    pub nmid_prc: String,        // idx 62 NXT 중간가
    pub nmid_total_rsqn: String, // idx 63 NXT 중간가 총잔량
    pub nmid_cls_code: String,   // idx 64 NXT 중간가 구분코드
}

/// 체결통보 (H0STCNI0/H0STCNI9) — 체결/접수 공통 26필드.
/// idx 13(체결여부)이 "2"면 체결, 그 외면 접수. idx 9·10·25 의미가
/// 유형에 따라 바뀜 (docs/kis-api/realtime.md §B-3 (a)/(b)).
#[derive(Debug, Clone)]
pub struct OrderNotice {
    pub cust_id: String,              // idx 0 고객 ID
    pub acnt_no: String,              // idx 1 계좌번호
    pub oder_no: String,              // idx 2 주문번호
    pub ooder_no: String,             // idx 3 원주문번호
    pub seln_byov_cls: String,        // idx 4 매도매수구분
    pub rctf_cls: String,             // idx 5 정정구분
    pub oder_kind: String,            // idx 6 주문종류
    pub oder_cond: String,            // idx 7 주문조건
    pub stck_shrn_iscd: String,       // idx 8 주식 단축 종목코드
    pub cntg_qty: String,             // idx 9 체결수량
    pub cntg_unpr: String,            // idx 10 체결단가
    pub stck_cntg_hour: String,       // idx 11 주식 체결시간
    pub rfus_yn: String,              // idx 12 거부여부
    pub cntg_yn: String,              // idx 13 체결여부 (2=체결/그외=접수)
    pub acpt_yn: String,              // idx 14 접수여부
    pub brnc_no: String,              // idx 15 지점번호
    pub oder_qty: String,             // idx 16 주문수량
    pub acnt_name: String,            // idx 17 계좌명
    pub askp_cond_prc: String,        // idx 18 호가조건가격
    pub ordg_excg_dvsn: String,       // idx 19 주문거래소 구분
    pub rt_cntg_scrn_disp_yn: String, // idx 20 실시간체결창 표시여부
    pub filler: String,               // idx 21 필러
    pub crdt_cls: String,             // idx 22 신용구분
    pub crdt_loan_date: String,       // idx 23 신용대출일자
    pub cntg_isnm40: String,          // idx 24 체결종목명40
    pub oder_prc: String,             // idx 25 주문가격
}

/// 해외주식 실시간체결가 (HDFSCNT0). 26개 필드.
/// 필드표 전체는 docs/kis-api/realtime.md §B-4 — 순번 1~26 verbatim.
#[derive(Debug, Clone)]
pub struct OverseasTrade {
    pub rsym: String, // 1 실시간 종목코드
    pub symb: String, // 2 종목코드
    pub zdiv: String, // 3 소수점 자리수
    pub tymd: String, // 4 현지영업일자
    pub xymd: String, // 5 현지일자
    pub xhms: String, // 6 현지시간
    pub kymd: String, // 7 한국일자
    pub khms: String, // 8 한국시간
    pub open: String, // 9 시가
    pub high: String, // 10 고가
    pub low: String,  // 11 저가
    pub last: String, // 12 현재가
    pub sign: String, // 13 대비구분
    pub diff: String, // 14 전일대비
    pub rate: String, // 15 등락율
    pub pbid: String, // 16 매수호가
    pub pask: String, // 17 매도호가
    pub vbid: String, // 18 매수잔량
    pub vask: String, // 19 매도잔량
    pub evol: String, // 20 체결량
    pub tvol: String, // 21 거래량
    pub tamt: String, // 22 거래대금
    pub bivl: String, // 23 매도체결량
    pub asvl: String, // 24 매수체결량
    pub strn: String, // 25 체결강도
    pub mtyp: String, // 26 시장구분
}

/// 국내주식 실시간 장운영 (H0{ST,NX,UN}MKO0). ST/NX 11필드, UN 10필드(종목코드 없음).
/// 필드 순서 원천: docs/kis-api/nxt-ws-columns.txt — 와이어 미검증.
#[derive(Debug, Clone)]
pub struct MarketOperation {
    pub mksc_shrn_iscd: String,     // 종목코드 (UN 통합은 빈값)
    pub trht_yn: String,            // 거래정지여부
    pub tr_susp_reas_cntt: String,  // 거래정지사유내용
    pub mkop_cls_code: String,      // 장운영구분코드
    pub antc_mkop_cls_code: String, // 예상장운영구분코드
    pub mrkt_trtm_cls_code: String, // 임의종료구분코드
    pub divi_app_cls_code: String,  // 동시호가배분처리구분코드
    pub iscd_stat_cls_code: String, // 종목상태구분코드
    pub vi_cls_code: String,        // VI적용구분코드
    pub ovtm_vi_cls_code: String,   // 시간외VI적용구분코드
    pub exch_cls_code: String,      // 거래소구분코드
}

/// 국내주식 실시간 프로그램매매 (H0{ST,NX,UN}PGM0). 11필드.
/// 필드 순서 원천: docs/kis-api/nxt-ws-columns.txt — 와이어 미검증.
#[derive(Debug, Clone)]
pub struct ProgramTrade {
    pub mksc_shrn_iscd: String, // 종목코드
    pub stck_cntg_hour: String, // 체결시간
    pub seln_cnqn: String,      // 매도체결수량
    pub seln_tr_pbmn: String,   // 매도거래대금
    pub shnu_cnqn: String,      // 매수체결수량
    pub shnu_tr_pbmn: String,   // 매수거래대금
    pub ntby_cnqn: String,      // 순매수체결수량
    pub ntby_tr_pbmn: String,   // 순매수거래대금
    pub seln_rsqn: String,      // 매도잔량
    pub shnu_rsqn: String,      // 매수잔량
    pub whol_ntby_qty: String,  // 전체순매수수량
}

/// 국내주식 실시간 회원사 (H0{ST,NX,UN}MBC0). 78필드.
/// 필드 순서 원천: docs/kis-api/nxt-ws-columns.txt — 와이어 미검증.
#[derive(Debug, Clone)]
pub struct MemberTrade {
    pub mksc_shrn_iscd: String,
    pub seln2_mbcr_name1: String,
    pub seln2_mbcr_name2: String,
    pub seln2_mbcr_name3: String,
    pub seln2_mbcr_name4: String,
    pub seln2_mbcr_name5: String,
    pub byov_mbcr_name1: String,
    pub byov_mbcr_name2: String,
    pub byov_mbcr_name3: String,
    pub byov_mbcr_name4: String,
    pub byov_mbcr_name5: String,
    pub total_seln_qty1: String,
    pub total_seln_qty2: String,
    pub total_seln_qty3: String,
    pub total_seln_qty4: String,
    pub total_seln_qty5: String,
    pub total_shnu_qty1: String,
    pub total_shnu_qty2: String,
    pub total_shnu_qty3: String,
    pub total_shnu_qty4: String,
    pub total_shnu_qty5: String,
    pub seln_mbcr_glob_yn_1: String,
    pub seln_mbcr_glob_yn_2: String,
    pub seln_mbcr_glob_yn_3: String,
    pub seln_mbcr_glob_yn_4: String,
    pub seln_mbcr_glob_yn_5: String,
    pub shnu_mbcr_glob_yn_1: String,
    pub shnu_mbcr_glob_yn_2: String,
    pub shnu_mbcr_glob_yn_3: String,
    pub shnu_mbcr_glob_yn_4: String,
    pub shnu_mbcr_glob_yn_5: String,
    pub seln_mbcr_no1: String,
    pub seln_mbcr_no2: String,
    pub seln_mbcr_no3: String,
    pub seln_mbcr_no4: String,
    pub seln_mbcr_no5: String,
    pub shnu_mbcr_no1: String,
    pub shnu_mbcr_no2: String,
    pub shnu_mbcr_no3: String,
    pub shnu_mbcr_no4: String,
    pub shnu_mbcr_no5: String,
    pub seln_mbcr_rlim1: String,
    pub seln_mbcr_rlim2: String,
    pub seln_mbcr_rlim3: String,
    pub seln_mbcr_rlim4: String,
    pub seln_mbcr_rlim5: String,
    pub shnu_mbcr_rlim1: String,
    pub shnu_mbcr_rlim2: String,
    pub shnu_mbcr_rlim3: String,
    pub shnu_mbcr_rlim4: String,
    pub shnu_mbcr_rlim5: String,
    pub seln_qty_icdc1: String,
    pub seln_qty_icdc2: String,
    pub seln_qty_icdc3: String,
    pub seln_qty_icdc4: String,
    pub seln_qty_icdc5: String,
    pub shnu_qty_icdc1: String,
    pub shnu_qty_icdc2: String,
    pub shnu_qty_icdc3: String,
    pub shnu_qty_icdc4: String,
    pub shnu_qty_icdc5: String,
    pub glob_total_seln_qty: String,
    pub glob_total_shnu_qty: String,
    pub glob_total_seln_qty_icdc: String,
    pub glob_total_shnu_qty_icdc: String,
    pub glob_ntby_qty: String,
    pub glob_seln_rlim: String,
    pub glob_shnu_rlim: String,
    pub seln2_mbcr_eng_name1: String,
    pub seln2_mbcr_eng_name2: String,
    pub seln2_mbcr_eng_name3: String,
    pub seln2_mbcr_eng_name4: String,
    pub seln2_mbcr_eng_name5: String,
    pub byov_mbcr_eng_name1: String,
    pub byov_mbcr_eng_name2: String,
    pub byov_mbcr_eng_name3: String,
    pub byov_mbcr_eng_name4: String,
    pub byov_mbcr_eng_name5: String,
}

/// tr_id별 `^` 분해 후 한 건당 필드 수. KRX(ST)/NXT(NX)/통합(UN) 비균일 주의.
fn fields_per_record(tr_id: &str) -> Option<usize> {
    match tr_id {
        // 체결가 CNT — 3시장 동일 46.
        "H0STCNT0" | "H0NXCNT0" | "H0UNCNT0" => Some(46),
        // 호가 ASP — KRX 59, NXT/통합 65 (중간가 6필드 추가).
        "H0STASP0" => Some(59),
        "H0NXASP0" | "H0UNASP0" => Some(65),
        // 예상체결 ANC — KRX 45, NXT/통합 46 (VI 기준가 추가).
        "H0STANC0" => Some(45),
        "H0NXANC0" | "H0UNANC0" => Some(46),
        // 장운영 MKO — ST/NX 11, 통합 10 (종목코드 없음).
        "H0STMKO0" | "H0NXMKO0" => Some(11),
        "H0UNMKO0" => Some(10),
        // 회원사 MBC — 3시장 동일 78.
        "H0STMBC0" | "H0NXMBC0" | "H0UNMBC0" => Some(78),
        // 프로그램매매 PGM — 3시장 동일 11.
        "H0STPGM0" | "H0NXPGM0" | "H0UNPGM0" => Some(11),
        // 체결통보 / 해외체결가 — 불변.
        "H0STCNI0" | "H0STCNI9" => Some(26),
        "HDFSCNT0" => Some(26),
        _ => None,
    }
}

/// 한 수신 프레임을 디코드해 0건 이상의 이벤트로 변환.
///
/// `creds`: 체결통보 tr_id의 AES key/iv. 평문 tr_id이면 무시.
/// 체결통보인데 `None`이면 빈 Vec 반환; 호출자가 warn 후 드롭한다 (D4).
pub(crate) fn decode_frame(raw: &str, creds: Option<&AesCreds>) -> Result<Vec<DecodedRecord>> {
    let parts: Vec<&str> = raw.split('|').collect();
    if parts.len() < 4 {
        return Err(KisError::Decode(format!(
            "realtime frame has {} segments, expected >=4",
            parts.len()
        )));
    }
    let encrypted = parts[0] == "1";
    let tr_id = parts[1];
    let data_cnt: usize = parts[2]
        .trim()
        .parse()
        .map_err(|e| KisError::Decode(format!("invalid data_cnt {}: {e}", parts[2])))?;

    let body_owned;
    let body: &str = if encrypted {
        let creds = match creds {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };
        body_owned = creds.decrypt(parts[3])?;
        &body_owned
    } else {
        parts[3]
    };

    let n_fields = fields_per_record(tr_id)
        .ok_or_else(|| KisError::Decode(format!("unsupported tr_id {tr_id}")))?;
    let fields: Vec<&str> = body.split('^').collect();
    let expected = n_fields * data_cnt.max(1);
    if fields.len() < expected {
        return Err(KisError::Decode(format!(
            "tr_id {tr_id}: got {} fields, expected {} ({}x{})",
            fields.len(),
            expected,
            n_fields,
            data_cnt
        )));
    }

    let mut out = Vec::with_capacity(data_cnt.max(1));
    for chunk in fields.chunks(n_fields).take(data_cnt.max(1)) {
        out.push(record_from_fields(tr_id, chunk)?);
    }
    Ok(out)
}

/// `^` 분해된 한 건의 필드 슬라이스를 tr_id별 struct로.
// 실시간 핫패스 — variant마다 큰 레코드이나 per-tick 힙 할당 회피 위해 박싱 안 함.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum DecodedRecord {
    StockTrade(StockTrade),
    /// 예상체결 (ANC) — 체결가와 동일 레이아웃, 별도 이벤트.
    ExpectedConclusion(StockTrade),
    StockAsking(StockAsking),
    MarketOperation(MarketOperation),
    MemberTrade(MemberTrade),
    ProgramTrade(ProgramTrade),
    OrderNotice(OrderNotice),
    OverseasTrade(OverseasTrade),
}

fn record_from_fields(tr_id: &str, f: &[&str]) -> Result<DecodedRecord> {
    // 비균일 레이아웃(ASP 59/65, ANC 45/46, MKO 11/10) 안전 읽기 — OOB는 빈 String.
    let g = |i: usize| f.get(i).map(|s| s.to_string()).unwrap_or_default();
    // 체결가(CNT)·예상체결(ANC)는 동일 46필드 레이아웃 — 빌더 1개 재사용.
    // ANC KRX(45필드)는 idx45 vi_stnd_prc 없음 → 안전 getter가 빈 String 처리.
    let stock_trade = || StockTrade {
        mksc_shrn_iscd: g(0),
            stck_cntg_hour: g(1),
            stck_prpr: g(2),
            prdy_vrss_sign: g(3),
            prdy_vrss: g(4),
            prdy_ctrt: g(5),
            wghn_avrg_stck_prc: g(6),
            stck_oprc: g(7),
            stck_hgpr: g(8),
            stck_lwpr: g(9),
            askp1: g(10),
            bidp1: g(11),
            cntg_vol: g(12),
            acml_vol: g(13),
            acml_tr_pbmn: g(14),
            seln_cntg_csnu: g(15),
            shnu_cntg_csnu: g(16),
            ntby_cntg_csnu: g(17),
            cttr: g(18),
            seln_cntg_smtn: g(19),
            shnu_cntg_smtn: g(20),
            ccld_dvsn: g(21),
            shnu_rate: g(22),
            prdy_vol_vrss_acml_vol_rate: g(23),
            oprc_hour: g(24),
            oprc_vrss_prpr_sign: g(25),
            oprc_vrss_prpr: g(26),
            hgpr_hour: g(27),
            hgpr_vrss_prpr_sign: g(28),
            hgpr_vrss_prpr: g(29),
            lwpr_hour: g(30),
            lwpr_vrss_prpr_sign: g(31),
            lwpr_vrss_prpr: g(32),
            bsop_date: g(33),
            new_mkop_cls_code: g(34),
            trht_yn: g(35),
            askp_rsqn1: g(36),
            bidp_rsqn1: g(37),
            total_askp_rsqn: g(38),
            total_bidp_rsqn: g(39),
            vol_tnrt: g(40),
            prdy_smns_hour_acml_vol: g(41),
            prdy_smns_hour_acml_vol_rate: g(42),
            hour_cls_code: g(43),
            mrkt_trtm_cls_code: g(44),
            vi_stnd_prc: g(45),
    };
    Ok(match tr_id {
        "H0STCNT0" | "H0NXCNT0" | "H0UNCNT0" => DecodedRecord::StockTrade(stock_trade()),
        "H0STANC0" | "H0NXANC0" | "H0UNANC0" => {
            DecodedRecord::ExpectedConclusion(stock_trade())
        }
        "H0STASP0" | "H0NXASP0" | "H0UNASP0" => DecodedRecord::StockAsking(StockAsking {
            mksc_shrn_iscd: g(0),
            bsop_hour: g(1),
            hour_cls_code: g(2),
            askp1: g(3),
            askp2: g(4),
            askp3: g(5),
            askp4: g(6),
            askp5: g(7),
            askp6: g(8),
            askp7: g(9),
            askp8: g(10),
            askp9: g(11),
            askp10: g(12),
            bidp1: g(13),
            bidp2: g(14),
            bidp3: g(15),
            bidp4: g(16),
            bidp5: g(17),
            bidp6: g(18),
            bidp7: g(19),
            bidp8: g(20),
            bidp9: g(21),
            bidp10: g(22),
            askp_rsqn1: g(23),
            askp_rsqn2: g(24),
            askp_rsqn3: g(25),
            askp_rsqn4: g(26),
            askp_rsqn5: g(27),
            askp_rsqn6: g(28),
            askp_rsqn7: g(29),
            askp_rsqn8: g(30),
            askp_rsqn9: g(31),
            askp_rsqn10: g(32),
            bidp_rsqn1: g(33),
            bidp_rsqn2: g(34),
            bidp_rsqn3: g(35),
            bidp_rsqn4: g(36),
            bidp_rsqn5: g(37),
            bidp_rsqn6: g(38),
            bidp_rsqn7: g(39),
            bidp_rsqn8: g(40),
            bidp_rsqn9: g(41),
            bidp_rsqn10: g(42),
            total_askp_rsqn: g(43),
            total_bidp_rsqn: g(44),
            ovtm_total_askp_rsqn: g(45),
            ovtm_total_bidp_rsqn: g(46),
            antc_cnpr: g(47),
            antc_cntg_vol: g(48),
            antc_vol: g(49),
            antc_cntg_vrss: g(50),
            antc_cntg_vrss_sign: g(51),
            antc_cntg_prdy_ctrt: g(52),
            acml_vol: g(53),
            total_askp_rsqn_icdc: g(54),
            total_bidp_rsqn_icdc: g(55),
            ovtm_total_askp_rsqn_icdc: g(56),
            ovtm_total_bidp_rsqn_icdc: g(57),
            stck_bsop_cls_code: g(58),
            kmid_prc: g(59),
            kmid_total_rsqn: g(60),
            kmid_cls_code: g(61),
            nmid_prc: g(62),
            nmid_total_rsqn: g(63),
            nmid_cls_code: g(64),
        }),
        "H0STCNI0" | "H0STCNI9" => DecodedRecord::OrderNotice(OrderNotice {
            cust_id: g(0),
            acnt_no: g(1),
            oder_no: g(2),
            ooder_no: g(3),
            seln_byov_cls: g(4),
            rctf_cls: g(5),
            oder_kind: g(6),
            oder_cond: g(7),
            stck_shrn_iscd: g(8),
            cntg_qty: g(9),
            cntg_unpr: g(10),
            stck_cntg_hour: g(11),
            rfus_yn: g(12),
            cntg_yn: g(13),
            acpt_yn: g(14),
            brnc_no: g(15),
            oder_qty: g(16),
            acnt_name: g(17),
            askp_cond_prc: g(18),
            ordg_excg_dvsn: g(19),
            rt_cntg_scrn_disp_yn: g(20),
            filler: g(21),
            crdt_cls: g(22),
            crdt_loan_date: g(23),
            cntg_isnm40: g(24),
            oder_prc: g(25),
        }),
        "HDFSCNT0" => DecodedRecord::OverseasTrade(OverseasTrade {
            rsym: g(0),
            symb: g(1),
            zdiv: g(2),
            tymd: g(3),
            xymd: g(4),
            xhms: g(5),
            kymd: g(6),
            khms: g(7),
            open: g(8),
            high: g(9),
            low: g(10),
            last: g(11),
            sign: g(12),
            diff: g(13),
            rate: g(14),
            pbid: g(15),
            pask: g(16),
            vbid: g(17),
            vask: g(18),
            evol: g(19),
            tvol: g(20),
            tamt: g(21),
            bivl: g(22),
            asvl: g(23),
            strn: g(24),
            mtyp: g(25),
        }),
        "H0STMKO0" | "H0NXMKO0" | "H0UNMKO0" => {
            // 통합(UN)은 종목코드 필드 없음 → idx 1칸 당겨짐.
            let un = tr_id == "H0UNMKO0";
            let o = usize::from(!un);
            DecodedRecord::MarketOperation(MarketOperation {
                mksc_shrn_iscd: if un { String::new() } else { g(0) },
                trht_yn: g(o),
                tr_susp_reas_cntt: g(o + 1),
                mkop_cls_code: g(o + 2),
                antc_mkop_cls_code: g(o + 3),
                mrkt_trtm_cls_code: g(o + 4),
                divi_app_cls_code: g(o + 5),
                iscd_stat_cls_code: g(o + 6),
                vi_cls_code: g(o + 7),
                ovtm_vi_cls_code: g(o + 8),
                exch_cls_code: g(o + 9),
            })
        }
        "H0STPGM0" | "H0NXPGM0" | "H0UNPGM0" => DecodedRecord::ProgramTrade(ProgramTrade {
            mksc_shrn_iscd: g(0),
            stck_cntg_hour: g(1),
            seln_cnqn: g(2),
            seln_tr_pbmn: g(3),
            shnu_cnqn: g(4),
            shnu_tr_pbmn: g(5),
            ntby_cnqn: g(6),
            ntby_tr_pbmn: g(7),
            seln_rsqn: g(8),
            shnu_rsqn: g(9),
            whol_ntby_qty: g(10),
        }),
        "H0STMBC0" | "H0NXMBC0" | "H0UNMBC0" => DecodedRecord::MemberTrade(MemberTrade {
            mksc_shrn_iscd: g(0),
            seln2_mbcr_name1: g(1),
            seln2_mbcr_name2: g(2),
            seln2_mbcr_name3: g(3),
            seln2_mbcr_name4: g(4),
            seln2_mbcr_name5: g(5),
            byov_mbcr_name1: g(6),
            byov_mbcr_name2: g(7),
            byov_mbcr_name3: g(8),
            byov_mbcr_name4: g(9),
            byov_mbcr_name5: g(10),
            total_seln_qty1: g(11),
            total_seln_qty2: g(12),
            total_seln_qty3: g(13),
            total_seln_qty4: g(14),
            total_seln_qty5: g(15),
            total_shnu_qty1: g(16),
            total_shnu_qty2: g(17),
            total_shnu_qty3: g(18),
            total_shnu_qty4: g(19),
            total_shnu_qty5: g(20),
            seln_mbcr_glob_yn_1: g(21),
            seln_mbcr_glob_yn_2: g(22),
            seln_mbcr_glob_yn_3: g(23),
            seln_mbcr_glob_yn_4: g(24),
            seln_mbcr_glob_yn_5: g(25),
            shnu_mbcr_glob_yn_1: g(26),
            shnu_mbcr_glob_yn_2: g(27),
            shnu_mbcr_glob_yn_3: g(28),
            shnu_mbcr_glob_yn_4: g(29),
            shnu_mbcr_glob_yn_5: g(30),
            seln_mbcr_no1: g(31),
            seln_mbcr_no2: g(32),
            seln_mbcr_no3: g(33),
            seln_mbcr_no4: g(34),
            seln_mbcr_no5: g(35),
            shnu_mbcr_no1: g(36),
            shnu_mbcr_no2: g(37),
            shnu_mbcr_no3: g(38),
            shnu_mbcr_no4: g(39),
            shnu_mbcr_no5: g(40),
            seln_mbcr_rlim1: g(41),
            seln_mbcr_rlim2: g(42),
            seln_mbcr_rlim3: g(43),
            seln_mbcr_rlim4: g(44),
            seln_mbcr_rlim5: g(45),
            shnu_mbcr_rlim1: g(46),
            shnu_mbcr_rlim2: g(47),
            shnu_mbcr_rlim3: g(48),
            shnu_mbcr_rlim4: g(49),
            shnu_mbcr_rlim5: g(50),
            seln_qty_icdc1: g(51),
            seln_qty_icdc2: g(52),
            seln_qty_icdc3: g(53),
            seln_qty_icdc4: g(54),
            seln_qty_icdc5: g(55),
            shnu_qty_icdc1: g(56),
            shnu_qty_icdc2: g(57),
            shnu_qty_icdc3: g(58),
            shnu_qty_icdc4: g(59),
            shnu_qty_icdc5: g(60),
            glob_total_seln_qty: g(61),
            glob_total_shnu_qty: g(62),
            glob_total_seln_qty_icdc: g(63),
            glob_total_shnu_qty_icdc: g(64),
            glob_ntby_qty: g(65),
            glob_seln_rlim: g(66),
            glob_shnu_rlim: g(67),
            seln2_mbcr_eng_name1: g(68),
            seln2_mbcr_eng_name2: g(69),
            seln2_mbcr_eng_name3: g(70),
            seln2_mbcr_eng_name4: g(71),
            seln2_mbcr_eng_name5: g(72),
            byov_mbcr_eng_name1: g(73),
            byov_mbcr_eng_name2: g(74),
            byov_mbcr_eng_name3: g(75),
            byov_mbcr_eng_name4: g(76),
            byov_mbcr_eng_name5: g(77),
        }),
        other => return Err(KisError::Decode(format!("unsupported tr_id {other}"))),
    })
}

/// JSON 제어 메시지 파싱 결과.
#[derive(Debug, Clone)]
pub(crate) enum ControlMessage {
    /// PINGPONG — 받은 raw를 그대로 pong으로 되돌려야 함.
    PingPong,
    /// 구독 응답. rt_cd=="0"이면 ok. 체결통보면 key/iv 동봉.
    SubscribeAck {
        tr_id: String,
        tr_key: String,
        rt_cd: String,
        msg: String,
        creds: Option<AesCreds>,
    },
}

/// 첫 글자가 `0`/`1`이 아닌 텍스트 프레임(JSON)을 파싱.
pub(crate) fn decode_control(raw: &str) -> Result<ControlMessage> {
    let v: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| KisError::Decode(format!("control json parse: {e}")))?;
    let tr_id = v
        .pointer("/header/tr_id")
        .and_then(|x| x.as_str())
        .unwrap_or_default();
    if tr_id == "PINGPONG" {
        return Ok(ControlMessage::PingPong);
    }
    let tr_key = v
        .pointer("/header/tr_key")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let body_str = |p: &str| {
        v.pointer(p)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let creds = match (
        v.pointer("/body/output/key").and_then(|x| x.as_str()),
        v.pointer("/body/output/iv").and_then(|x| x.as_str()),
    ) {
        (Some(k), Some(iv)) => Some(AesCreds {
            key: k.to_string(),
            iv: iv.to_string(),
        }),
        _ => None,
    };
    Ok(ControlMessage::SubscribeAck {
        tr_id: tr_id.to_string(),
        tr_key,
        rt_cd: body_str("/body/rt_cd"),
        msg: body_str("/body/msg1"),
        creds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_frame_single_record() {
        let body: String = (0..46).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0STCNT0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        assert_eq!(recs.len(), 1);
        match &recs[0] {
            DecodedRecord::StockTrade(t) => assert_eq!(t.stck_prpr, "2"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn plain_frame_multi_record() {
        let body: String = (0..92).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0STCNT0|002|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        assert_eq!(recs.len(), 2, "다건 프레임은 건수만큼 이벤트");
    }

    #[test]
    fn nxt_asking_has_mid_price_tail() {
        // NXT 호가 65필드 — idx59~64 중간가 채워짐.
        let body: String = (0..65).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0NXASP0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        match &recs[0] {
            DecodedRecord::StockAsking(a) => {
                assert_eq!(a.kmid_prc, "59");
                assert_eq!(a.nmid_cls_code, "64");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn krx_asking_mid_price_tail_is_empty() {
        // KRX 호가 59필드 — 중간가 필드 없음 → 안전 getter가 빈 String (OOB 패닉 없음).
        let body: String = (0..59).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0STASP0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        match &recs[0] {
            DecodedRecord::StockAsking(a) => {
                assert_eq!(a.stck_bsop_cls_code, "58");
                assert!(a.kmid_prc.is_empty());
                assert!(a.nmid_cls_code.is_empty());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn unified_market_op_drops_symbol_offset() {
        // 통합 장운영 10필드 — 종목코드 없이 trht_yn부터 시작.
        let body: String = (0..10).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0UNMKO0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        match &recs[0] {
            DecodedRecord::MarketOperation(m) => {
                assert!(m.mksc_shrn_iscd.is_empty(), "통합은 종목코드 없음");
                assert_eq!(m.trht_yn, "0", "idx0이 trht_yn으로 당겨짐");
                assert_eq!(m.exch_cls_code, "9");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn nxt_market_op_keeps_symbol() {
        // NXT 장운영 11필드 — 종목코드 idx0 유지.
        let body: String = (0..11).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0NXMKO0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        match &recs[0] {
            DecodedRecord::MarketOperation(m) => {
                assert_eq!(m.mksc_shrn_iscd, "0");
                assert_eq!(m.trht_yn, "1");
                assert_eq!(m.exch_cls_code, "10");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn anc_expected_conclusion_variant() {
        // 예상체결 — 체결가와 동일 레이아웃, 별도 변형.
        let body: String = (0..46).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0NXANC0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        assert!(matches!(recs[0], DecodedRecord::ExpectedConclusion(_)));
    }

    #[test]
    fn nxt_trade_reuses_stock_trade() {
        let body: String = (0..46).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0UNCNT0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        assert!(matches!(recs[0], DecodedRecord::StockTrade(_)));
    }

    #[test]
    fn member_trade_78_fields() {
        let body: String = (0..78).map(|i| i.to_string()).collect::<Vec<_>>().join("^");
        let raw = format!("0|H0NXMBC0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        match &recs[0] {
            DecodedRecord::MemberTrade(m) => {
                assert_eq!(m.mksc_shrn_iscd, "0");
                assert_eq!(m.byov_mbcr_eng_name5, "77");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn encrypted_frame_without_creds_is_dropped() {
        let raw = "1|H0STCNI0|001|ciphertext-not-read-without-creds";
        let recs = decode_frame(raw, None).unwrap();
        assert!(recs.is_empty(), "key/iv 미도착 시 드롭");
    }

    #[test]
    fn pingpong_control() {
        let raw = r#"{"header":{"tr_id":"PINGPONG"}}"#;
        assert!(matches!(
            decode_control(raw).unwrap(),
            ControlMessage::PingPong
        ));
    }

    #[test]
    fn subscribe_ack_with_creds() {
        let raw = r#"{"header":{"tr_id":"H0STCNI0","tr_key":"htsid"},
            "body":{"rt_cd":"0","msg1":"OK",
            "output":{"key":"k","iv":"v"}}}"#;
        match decode_control(raw).unwrap() {
            ControlMessage::SubscribeAck {
                creds,
                rt_cd,
                tr_key,
                ..
            } => {
                assert_eq!(rt_cd, "0");
                assert_eq!(tr_key, "htsid");
                assert!(creds.is_some());
            }
            _ => panic!("wrong variant"),
        }
    }
}
