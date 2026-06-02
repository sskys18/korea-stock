//! 구독/해지 프레임 + SubscriptionHandle.
//!
//! 구독 프레임 구조: docs/kis-api/realtime.md §A-3.
//! tr_type "1"=등록(구독), "2"=등록해제(해지).

use tokio::sync::mpsc;

use crate::kis::domestic_stock::Market;

/// 구독 가능한 실시간 데이터 종류.
///
/// 국내주식 6종 스트림은 `Market`(KRX/NXT/통합)로 거래소를 선택 — tr_id는
/// `H0{infix}{suffix}0` 형태로 합성된다(예: NXT 호가 `H0NXASP0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionKind {
    /// 실시간체결가 (H0{ST,NX,UN}CNT0). tr_key=종목코드.
    DomesticTrade(Market),
    /// 실시간호가 (H0{ST,NX,UN}ASP0). tr_key=종목코드.
    DomesticAsking(Market),
    /// 실시간예상체결 (H0{ST,NX,UN}ANC0). tr_key=종목코드.
    ExpectedConclusion(Market),
    /// 실시간장운영 (H0{ST,NX,UN}MKO0). tr_key=종목코드.
    MarketOperation(Market),
    /// 실시간회원사 (H0{ST,NX,UN}MBC0). tr_key=종목코드.
    MemberTrade(Market),
    /// 실시간프로그램매매 (H0{ST,NX,UN}PGM0). tr_key=종목코드.
    ProgramTrade(Market),
    /// H0STCNI0(실전)/H0STCNI9(모의) — 체결통보. tr_key=HTS ID.
    OrderNotice,
    /// HDFSCNT0 — 해외주식 실시간체결가. tr_key=실시간종목코드.
    OverseasTrade,
}

impl SubscriptionKind {
    /// 환경별 tr_id. 국내 6종은 `Market`별 합성, OrderNotice만 실전/모의가 다름.
    pub(crate) fn tr_id(self, env: crate::kis::config::Environment) -> String {
        use crate::kis::config::Environment::*;
        use SubscriptionKind::*;
        let domestic = |m: Market, suffix: &str| format!("H0{}{suffix}0", m.ws_infix());
        match self {
            DomesticTrade(m) => domestic(m, "CNT"),
            DomesticAsking(m) => domestic(m, "ASP"),
            ExpectedConclusion(m) => domestic(m, "ANC"),
            MarketOperation(m) => domestic(m, "MKO"),
            MemberTrade(m) => domestic(m, "MBC"),
            ProgramTrade(m) => domestic(m, "PGM"),
            OrderNotice => match env {
                Real => "H0STCNI0",
                Mock => "H0STCNI9",
            }
            .to_string(),
            OverseasTrade => "HDFSCNT0".to_string(),
        }
    }

    /// 체결통보 여부 — AES 복호화 필요.
    pub fn is_encrypted(self) -> bool {
        matches!(self, SubscriptionKind::OrderNotice)
    }
}

/// 구독/해지 프레임 JSON 문자열 생성. tr_type "1"=구독, "2"=해지.
pub(crate) fn build_frame(
    approval_key: &str,
    tr_id: &str,
    tr_key: &str,
    subscribe: bool,
) -> String {
    serde_json::json!({
        "header": {
            "approval_key": approval_key,
            "custtype": "P",
            "tr_type": if subscribe { "1" } else { "2" },
            "content-type": "utf-8",
        },
        "body": {
            "input": { "tr_id": tr_id, "tr_key": tr_key }
        }
    })
    .to_string()
}

/// 백그라운드 writer 태스크로 보내는 제어 메시지.
#[derive(Debug, Clone)]
pub(crate) enum ControlMsg {
    Subscribe { tr_id: String, tr_key: String },
    Unsubscribe { tr_id: String, tr_key: String },
}

/// 활성 구독 핸들. Drop 시 자동 해지 프레임 전송 (best-effort).
///
/// 이벤트는 `RealtimeClient` 생성 시 받은 단일 `mpsc::Receiver`로 흐른다 —
/// 핸들은 이벤트를 직접 노출하지 않고 "구독 수명"만 관리한다.
pub struct SubscriptionHandle {
    tr_id: String,
    tr_key: String,
    control_tx: mpsc::UnboundedSender<ControlMsg>,
    /// unsubscribe() 명시 호출 시 true — Drop에서 중복 해지 방지.
    released: bool,
}

impl SubscriptionHandle {
    pub(crate) fn new(
        tr_id: String,
        tr_key: String,
        control_tx: mpsc::UnboundedSender<ControlMsg>,
    ) -> Self {
        Self {
            tr_id,
            tr_key,
            control_tx,
            released: false,
        }
    }

    /// 명시적 구독 해지. 해지 프레임 전송 요청을 큐에 넣는다.
    pub async fn unsubscribe(mut self) {
        let _ = self.control_tx.send(ControlMsg::Unsubscribe {
            tr_id: self.tr_id.clone(),
            tr_key: self.tr_key.clone(),
        });
        self.released = true;
    }
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let _ = self.control_tx.send(ControlMsg::Unsubscribe {
            tr_id: self.tr_id.clone(),
            tr_key: self.tr_key.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kis::config::Environment;

    #[test]
    fn order_notice_tr_id_per_env() {
        assert_eq!(
            SubscriptionKind::OrderNotice.tr_id(Environment::Real),
            "H0STCNI0"
        );
        assert_eq!(
            SubscriptionKind::OrderNotice.tr_id(Environment::Mock),
            "H0STCNI9"
        );
    }

    #[test]
    fn tr_id_synthesis_per_market() {
        use crate::kis::config::Environment::Real;
        assert_eq!(SubscriptionKind::DomesticTrade(Market::Krx).tr_id(Real), "H0STCNT0");
        assert_eq!(SubscriptionKind::DomesticTrade(Market::Nxt).tr_id(Real), "H0NXCNT0");
        assert_eq!(
            SubscriptionKind::DomesticAsking(Market::Unified).tr_id(Real),
            "H0UNASP0"
        );
        assert_eq!(
            SubscriptionKind::ExpectedConclusion(Market::Nxt).tr_id(Real),
            "H0NXANC0"
        );
        assert_eq!(
            SubscriptionKind::MarketOperation(Market::Unified).tr_id(Real),
            "H0UNMKO0"
        );
        assert_eq!(SubscriptionKind::MemberTrade(Market::Krx).tr_id(Real), "H0STMBC0");
        assert_eq!(SubscriptionKind::ProgramTrade(Market::Nxt).tr_id(Real), "H0NXPGM0");
    }

    #[test]
    fn subscribe_frame_shape() {
        let f = build_frame("ak", "H0STCNT0", "005930", true);
        let v: serde_json::Value = serde_json::from_str(&f).unwrap();
        assert_eq!(v["header"]["tr_type"], "1");
        assert_eq!(v["header"]["approval_key"], "ak");
        assert_eq!(v["body"]["input"]["tr_id"], "H0STCNT0");
        assert_eq!(v["body"]["input"]["tr_key"], "005930");
    }

    #[test]
    fn unsubscribe_frame_shape() {
        let f = build_frame("ak", "H0STCNT0", "005930", false);
        let v: serde_json::Value = serde_json::from_str(&f).unwrap();
        assert_eq!(v["header"]["tr_type"], "2");
    }

    #[test]
    fn drop_sends_unsubscribe() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        {
            let _h = SubscriptionHandle::new("H0STCNT0".into(), "005930".into(), tx);
        }
        match rx.try_recv().unwrap() {
            ControlMsg::Unsubscribe { tr_id, tr_key } => {
                assert_eq!(tr_id, "H0STCNT0");
                assert_eq!(tr_key, "005930");
            }
            _ => panic!("expected Unsubscribe"),
        }
    }
}
