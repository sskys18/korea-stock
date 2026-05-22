//! 구독/해지 프레임 + SubscriptionHandle.
//!
//! 구독 프레임 구조: docs/kis-api/realtime.md §A-3.
//! tr_type "1"=등록(구독), "2"=등록해제(해지).

use tokio::sync::mpsc;

/// 구독 가능한 실시간 데이터 종류. 본 Plan 지원 4종.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionKind {
    /// H0STCNT0 — 국내주식 실시간체결가. tr_key=종목코드.
    DomesticTrade,
    /// H0STASP0 — 국내주식 실시간호가. tr_key=종목코드.
    DomesticAsking,
    /// H0STCNI0(실전)/H0STCNI9(모의) — 체결통보. tr_key=HTS ID.
    OrderNotice,
    /// HDFSCNT0 — 해외주식 실시간체결가. tr_key=실시간종목코드.
    OverseasTrade,
}

impl SubscriptionKind {
    /// 환경별 tr_id. OrderNotice만 실전/모의가 다름.
    pub(crate) fn tr_id(self, env: crate::config::Environment) -> &'static str {
        use crate::config::Environment::*;
        match (self, env) {
            (SubscriptionKind::DomesticTrade, _) => "H0STCNT0",
            (SubscriptionKind::DomesticAsking, _) => "H0STASP0",
            (SubscriptionKind::OrderNotice, Real) => "H0STCNI0",
            (SubscriptionKind::OrderNotice, Mock) => "H0STCNI9",
            (SubscriptionKind::OverseasTrade, _) => "HDFSCNT0",
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
    use crate::config::Environment;

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
